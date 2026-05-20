//! CLI logic for `npd` — exposed both via the `riri-npd` binary and the
//! `riri-napi-npd` JS shim.

use anyhow::{Context, Result};
use clap::Parser;
use comfy_table::{Table, presets};
use console::style;
use riri_common::{LockfileVersions, PackageJsonFile, PackageManager, detect_lockfile};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_task_runner::{RendererMode, TaskRunner};
use riri_yarn::YarnProject;
use std::path::{Path, PathBuf};

use crate::{CatalogPin, DependencyKind, VersionToPin, pin_dependencies};

/// Resolved catalog state for one `npd` invocation.
struct CatalogPlan {
    pins: Vec<CatalogPin>,
    /// `None` when `--pin-catalog` is off, the project isn't pnpm, or there
    /// is no `pnpm-workspace.yaml`. Required for write-back on `-u`.
    source: Option<CatalogSource>,
}

struct CatalogSource {
    path: PathBuf,
    raw: String,
}

const EXIT_OK: i32 = 0;
const EXIT_PINS_PENDING: i32 = 1;
const EXIT_USAGE_ERROR: i32 = 2;

/// Pin range-based dependency specifiers to the exact versions resolved by
/// the lockfile.
#[derive(Debug, Parser)]
#[command(name = "npd", version, about)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Silent mode — no output.
    #[arg(short, long)]
    pub quiet: bool,

    /// Verbose output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Debug mode — detailed logging.
    #[arg(short, long)]
    pub debug: bool,

    /// Update package.json with pinned versions.
    #[arg(short, long)]
    pub update: bool,

    /// Output results as JSON.
    #[arg(long)]
    pub json: bool,

    /// Sort package.json keys (sort-package-json conventions); writes the file even without --update or pending pins.
    #[arg(long)]
    pub sort: bool,

    /// Create or update .npmrc with save-exact=true.
    #[arg(long)]
    pub enable_save_exact: bool,

    /// Resolve and report pnpm catalog entries from `pnpm-workspace.yaml`.
    /// On `-u`, rewrites the catalog entries in place. Requires a pnpm project.
    #[arg(long)]
    pub pin_catalog: bool,
}

fn renderer_mode(args: &Args) -> RendererMode {
    if args.quiet {
        RendererMode::Silent
    } else if args.verbose {
        RendererMode::Simple
    } else if args.debug {
        RendererMode::Verbose
    } else {
        RendererMode::Default
    }
}

fn load_lockfile(
    manager: &PackageManager,
    lockfile_path: &Path,
) -> Result<Box<dyn LockfileVersions>> {
    match manager {
        PackageManager::Npm => {
            let content = std::fs::read_to_string(lockfile_path)
                .with_context(|| format!("failed to read {}", lockfile_path.display()))?;
            let lock =
                NpmPackageLock::parse(&content).context("failed to parse package-lock.json")?;
            Ok(Box::new(lock))
        }
        PackageManager::Pnpm => {
            let content = std::fs::read_to_string(lockfile_path)
                .with_context(|| format!("failed to read {}", lockfile_path.display()))?;
            let lock = PnpmLockfile::parse(&content).context("failed to parse pnpm-lock.yaml")?;
            Ok(Box::new(lock))
        }
        PackageManager::Yarn => {
            let project_dir = lockfile_path.parent().unwrap_or_else(|| Path::new("."));
            let project = YarnProject::scan(project_dir).with_context(|| {
                format!("failed to scan {}/node_modules", project_dir.display())
            })?;
            Ok(Box::new(project))
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<i32> {
    let mode = renderer_mode(args);
    if args.debug {
        install_debug_subscriber();
    }
    let runner = TaskRunner::new(mode);
    let cwd = std::env::current_dir().context("failed to get current directory")?;

    maybe_enable_save_exact(args, &runner, &cwd)?;

    if let Some(project) = riri_workspace::detect(&cwd) {
        return run_workspace(args, &runner, &project);
    }

    let task = runner.task("Detecting lockfile...");
    let lockfile_result = match detect_lockfile(&cwd) {
        Ok(result) => {
            task.complete(&format!(
                "Detected {}",
                result
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
            result
        }
        Err(e) => {
            task.fail("Detecting lockfile");
            return Err(anyhow::anyhow!(e));
        }
    };

    let task = runner.task("Reading package.json...");
    let pkg_path = cwd.join("package.json");
    let mut pkg_file = match PackageJsonFile::read(&pkg_path) {
        Ok(file) => {
            task.complete("Read package.json");
            file
        }
        Err(e) => {
            task.fail("Reading package.json");
            return Err(anyhow::anyhow!(e));
        }
    };

    let task = runner.task("Parsing lockfile...");
    let lockfile = match load_lockfile(&lockfile_result.package_manager, &lockfile_result.path) {
        Ok(lock) => {
            task.complete("Parsed lockfile");
            lock
        }
        Err(e) => {
            task.fail("Parsing lockfile");
            return Err(e);
        }
    };

    let task = runner.task("Computing dependency pins...");
    let pins = pin_dependencies(&pkg_file.parsed, lockfile.as_ref())
        .map_err(|e| anyhow::anyhow!("pin_dependencies failed: {e}"))?;
    task.complete("Computed dependency pins");

    let CatalogPlan {
        pins: catalog_pins,
        source: catalog_source,
    } = if args.pin_catalog {
        resolve_catalog_pins(args, &runner, &lockfile_result, &cwd, lockfile.as_ref())?
    } else {
        CatalogPlan {
            pins: Vec::new(),
            source: None,
        }
    };

    if args.json {
        let mut all_pins: Vec<serde_json::Value> = pins
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "kind": p.kind.as_str(),
                    "from": p.current_range,
                    "to": p.pinned_version,
                })
            })
            .collect();
        for cp in &catalog_pins {
            all_pins.push(serde_json::json!({
                "name": cp.dep_name,
                "kind": "catalog",
                "catalog": cp.catalog_name,
                "from": cp.from,
                "to": cp.to,
            }));
        }
        let json_output = serde_json::json!({ "pins": all_pins });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(if pins.is_empty() && catalog_pins.is_empty() {
            EXIT_OK
        } else {
            EXIT_PINS_PENDING
        });
    }

    if pins.is_empty() && catalog_pins.is_empty() {
        if !args.quiet {
            eprintln!(
                "\n  All dependencies are already pinned {}",
                style(":)").green()
            );
        }
        if args.sort {
            let task = runner.task("Sorting package.json...");
            pkg_file
                .write_sorted()
                .context("failed to write package.json")?;
            task.complete("Sorted package.json");
        }
        return Ok(EXIT_OK);
    }

    if !args.quiet {
        let mut table = Table::new();
        table.load_preset(presets::NOTHING);
        for pin in &pins {
            table.add_row(vec![
                pin.name.clone(),
                pin.current_range.clone(),
                "\u{2192}".to_string(),
                pin.pinned_version.clone(),
                String::new(),
            ]);
        }
        for cp in &catalog_pins {
            let origin = match &cp.catalog_name {
                None => "(catalog)".to_string(),
                Some(name) => format!("(catalog:{name})"),
            };
            table.add_row(vec![
                cp.dep_name.clone(),
                cp.from.clone(),
                "\u{2192}".to_string(),
                cp.to.clone(),
                origin,
            ]);
        }
        for line in table.lines() {
            eprintln!("    {}", line.trim());
        }
    }

    if args.update {
        if !pins.is_empty() {
            let task = runner.task("Updating package.json...");
            apply_pins(&mut pkg_file, &pins);
            if args.sort {
                pkg_file
                    .write_sorted()
                    .context("failed to write package.json")?;
            } else {
                pkg_file.write().context("failed to write package.json")?;
            }
            task.complete("Updated package.json");
        } else if args.sort {
            let task = runner.task("Sorting package.json...");
            pkg_file
                .write_sorted()
                .context("failed to write package.json")?;
            task.complete("Sorted package.json");
        }
        if let Some(source) = catalog_source.as_ref()
            && !catalog_pins.is_empty()
        {
            let task = runner.task("Updating pnpm-workspace.yaml...");
            match apply_catalog_pins(source, &catalog_pins) {
                Ok(()) => task.complete("Updated pnpm-workspace.yaml"),
                Err(e) => {
                    task.fail("Updating pnpm-workspace.yaml");
                    return Err(e);
                }
            }
        }
    } else {
        if args.sort {
            let task = runner.task("Sorting package.json...");
            pkg_file
                .write_sorted()
                .context("failed to write package.json")?;
            task.complete("Sorted package.json");
        }
        if !args.quiet {
            let hint = generate_update_hint(args);
            eprintln!(
                "\n  Run {} to upgrade package.json{}.",
                style(hint).bold().cyan(),
                if args.pin_catalog {
                    " and pnpm-workspace.yaml"
                } else {
                    ""
                },
            );
        }
    }

    Ok(EXIT_PINS_PENDING)
}

fn install_debug_subscriber() {
    use std::io::IsTerminal;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("riri_npd=debug"));
    let layer = tracing_subscriber::fmt::layer()
        .event_format(IndentedFormat)
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init();
}

struct IndentedFormat;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for IndentedFormat
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        writer.write_str("    ")?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

fn maybe_enable_save_exact(args: &Args, runner: &TaskRunner, cwd: &Path) -> Result<()> {
    if !args.enable_save_exact {
        return Ok(());
    }
    let task = runner.task("Enabling save-exact in .npmrc...");
    match riri_common::upsert_npmrc_flag(cwd, "save-exact=true")
        .context("failed to write .npmrc")?
    {
        riri_common::NpmrcOutcome::AlreadySet => {
            task.skip("Enabling save-exact in .npmrc", "already enabled");
        }
        riri_common::NpmrcOutcome::Added => {
            task.complete("Enabled save-exact in .npmrc");
        }
    }
    Ok(())
}

fn apply_pins(pkg_file: &mut PackageJsonFile, pins: &[VersionToPin]) {
    let raw = &mut pkg_file.raw;
    for pin in pins {
        let Some(bucket) = raw.get_mut(pin.kind.as_str()) else {
            continue;
        };
        if let Some(map) = bucket.as_object_mut()
            && map.contains_key(&pin.name)
        {
            map.insert(
                pin.name.clone(),
                serde_json::Value::String(pin.pinned_version.clone()),
            );
        }
    }
    if let Some(parsed_deps) = pkg_file.parsed.dependencies.as_mut() {
        sync_typed(parsed_deps, pins, DependencyKind::Dependencies);
    }
    if let Some(parsed_dev) = pkg_file.parsed.dev_dependencies.as_mut() {
        sync_typed(parsed_dev, pins, DependencyKind::DevDependencies);
    }
    if let Some(parsed_opt) = pkg_file.parsed.optional_dependencies.as_mut() {
        sync_typed(parsed_opt, pins, DependencyKind::OptionalDependencies);
    }
}

/// Rewrites each catalog pin into the original `pnpm-workspace.yaml` raw
/// content and atomically writes it back to disk.
fn apply_catalog_pins(source: &CatalogSource, pins: &[CatalogPin]) -> Result<()> {
    let mut content = source.raw.clone();
    for pin in pins {
        content = riri_pnpm::catalog::PnpmCatalog::edit_line(
            &content,
            pin.catalog_name.as_deref(),
            &pin.dep_name,
            &pin.to,
        )
        .with_context(|| {
            format!(
                "failed to rewrite catalog entry `{}`{}",
                pin.dep_name,
                pin.catalog_name
                    .as_ref()
                    .map(|c| format!(" in catalog `{c}`"))
                    .unwrap_or_default()
            )
        })?;
    }

    let parent = source.path.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = parent.join(".pnpm-workspace.yaml.tmp");
    std::fs::write(&tmp_path, &content)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &source.path)
        .with_context(|| format!("failed to rename into {}", source.path.display()))?;
    Ok(())
}

fn sync_typed(
    target: &mut std::collections::HashMap<String, String>,
    pins: &[VersionToPin],
    kind: DependencyKind,
) {
    for pin in pins.iter().filter(|p| p.kind == kind) {
        target.insert(pin.name.clone(), pin.pinned_version.clone());
    }
}

fn generate_update_hint(args: &Args) -> String {
    let mut parts = vec!["npd".to_string()];
    if args.quiet {
        parts.push("-q".to_string());
    }
    if args.verbose {
        parts.push("-v".to_string());
    }
    if args.debug {
        parts.push("-d".to_string());
    }
    parts.push("-u".to_string());
    if args.pin_catalog {
        parts.push("--pin-catalog".to_string());
    }
    parts.join(" ")
}

fn run_workspace(
    args: &Args,
    runner: &TaskRunner,
    project: &riri_workspace::WorkspaceProject,
) -> Result<i32> {
    let members = project
        .members()
        .map_err(|e| anyhow::anyhow!("failed to enumerate workspace members: {e}"))?;

    if members.is_empty() {
        if !args.quiet {
            eprintln!("  no workspace members found");
        }
        return Ok(EXIT_OK);
    }

    let task = runner.task("Detecting lockfile...");
    let lockfile_result = match detect_lockfile(project.root()) {
        Ok(result) => {
            task.complete(&format!(
                "Detected {}",
                result
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
            result
        }
        Err(e) => {
            task.fail("Detecting lockfile");
            return Err(anyhow::anyhow!(e));
        }
    };

    let task = runner.task("Parsing lockfile...");
    let lockfile = match load_lockfile(&lockfile_result.package_manager, &lockfile_result.path) {
        Ok(lock) => {
            task.complete("Parsed lockfile");
            lock
        }
        Err(e) => {
            task.fail("Parsing lockfile");
            return Err(e);
        }
    };

    let mut per_member: Vec<(
        riri_workspace::WorkspaceMember,
        Vec<VersionToPin>,
        PackageJsonFile,
    )> = Vec::new();
    let mut worst = EXIT_OK;

    for member in members {
        let mut pkg_file = PackageJsonFile::read(&member.manifest_path)
            .map_err(|e| anyhow::anyhow!("{}: {e}", member.name))?;
        let pins = pin_dependencies(&pkg_file.parsed, lockfile.as_ref())
            .map_err(|e| anyhow::anyhow!("{}: pin_dependencies failed: {e}", member.name))?;
        if !pins.is_empty() {
            worst = EXIT_PINS_PENDING;
        }
        if args.update && !pins.is_empty() {
            apply_pins(&mut pkg_file, &pins);
            if args.sort {
                pkg_file
                    .write_sorted()
                    .map_err(|e| anyhow::anyhow!("{}: write failed: {e}", member.name))?;
            } else {
                pkg_file
                    .write()
                    .map_err(|e| anyhow::anyhow!("{}: write failed: {e}", member.name))?;
            }
        }
        per_member.push((member, pins, pkg_file));
    }

    let mut catalog_pins: Vec<CatalogPin> = Vec::new();
    if args.pin_catalog && lockfile_result.package_manager == PackageManager::Pnpm {
        let plan = resolve_catalog_pins(
            args,
            runner,
            &lockfile_result,
            project.root(),
            lockfile.as_ref(),
        )?;
        if !plan.pins.is_empty() {
            worst = EXIT_PINS_PENDING;
        }
        if args.update
            && let Some(source) = plan.source.as_ref()
            && !plan.pins.is_empty()
        {
            let task = runner.task("Updating pnpm-workspace.yaml...");
            match apply_catalog_pins(source, &plan.pins) {
                Ok(()) => task.complete("Updated pnpm-workspace.yaml"),
                Err(e) => {
                    task.fail("Updating pnpm-workspace.yaml");
                    return Err(e);
                }
            }
        }
        catalog_pins = plan.pins;
    }

    if args.json {
        emit_workspace_json(project.root(), &per_member, &catalog_pins);
    } else {
        emit_workspace_text(args, &per_member, &catalog_pins);
    }
    Ok(worst)
}

fn relative_manifest(root: &Path, manifest: &Path) -> String {
    manifest
        .strip_prefix(root)
        .unwrap_or(manifest)
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn emit_workspace_json(
    root: &Path,
    per_member: &[(
        riri_workspace::WorkspaceMember,
        Vec<VersionToPin>,
        PackageJsonFile,
    )],
    catalog_pins: &[CatalogPin],
) {
    let members: Vec<serde_json::Value> = per_member
        .iter()
        .map(|(m, pins, _)| {
            serde_json::json!({
                "name": m.name,
                "manifest": relative_manifest(root, &m.manifest_path),
                "pins": pins.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "kind": p.kind.as_str(),
                    "from": p.current_range,
                    "to": p.pinned_version,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let catalog: Vec<serde_json::Value> = catalog_pins
        .iter()
        .map(|cp| {
            serde_json::json!({
                "name": cp.dep_name,
                "kind": "catalog",
                "catalog": cp.catalog_name,
                "from": cp.from,
                "to": cp.to,
            })
        })
        .collect();
    let out = serde_json::json!({ "members": members, "catalog": catalog });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

fn emit_workspace_text(
    args: &Args,
    per_member: &[(
        riri_workspace::WorkspaceMember,
        Vec<VersionToPin>,
        PackageJsonFile,
    )],
    catalog_pins: &[CatalogPin],
) {
    if args.quiet {
        return;
    }
    let mut any = false;
    for (member, pins, _) in per_member {
        if pins.is_empty() {
            continue;
        }
        any = true;
        eprintln!("\n  {}:", member.name);
        let mut table = Table::new();
        table.load_preset(presets::NOTHING);
        for pin in pins {
            table.add_row(vec![
                pin.name.clone(),
                pin.current_range.clone(),
                "\u{2192}".to_string(),
                pin.pinned_version.clone(),
            ]);
        }
        for line in table.lines() {
            eprintln!("    {}", line.trim());
        }
    }
    if !catalog_pins.is_empty() {
        any = true;
        eprintln!("\n  (catalog):");
        let mut table = Table::new();
        table.load_preset(presets::NOTHING);
        for cp in catalog_pins {
            let label = match &cp.catalog_name {
                None => cp.dep_name.clone(),
                Some(name) => format!("{}:{}", name, cp.dep_name),
            };
            table.add_row(vec![
                label,
                cp.from.clone(),
                "\u{2192}".to_string(),
                cp.to.clone(),
            ]);
        }
        for line in table.lines() {
            eprintln!("    {}", line.trim());
        }
    }
    if !any {
        eprintln!(
            "\n  All workspace dependencies are already pinned {}",
            style(":)").green()
        );
    } else if !args.update {
        let hint = generate_update_hint(args);
        eprintln!(
            "\n  Run {} to upgrade {} package.json files.",
            style(hint).bold().cyan(),
            per_member.iter().filter(|(_, p, _)| !p.is_empty()).count(),
        );
    }
}

fn resolve_catalog_pins(
    args: &Args,
    runner: &TaskRunner,
    lockfile_result: &riri_common::LockFileResult,
    cwd: &Path,
    lockfile: &dyn LockfileVersions,
) -> Result<CatalogPlan> {
    if lockfile_result.package_manager != PackageManager::Pnpm {
        if !args.quiet {
            eprintln!(
                "  {} --pin-catalog is pnpm-only, ignoring",
                style("warning:").yellow().bold(),
            );
        }
        return Ok(CatalogPlan {
            pins: Vec::new(),
            source: None,
        });
    }

    let Some(yaml_path) = riri_pnpm::catalog::find_workspace_yaml(cwd) else {
        if !args.quiet {
            eprintln!(
                "  {} pnpm-workspace.yaml not found, no catalog entries to pin",
                style("warning:").yellow().bold(),
            );
        }
        return Ok(CatalogPlan {
            pins: Vec::new(),
            source: None,
        });
    };

    let task = runner.task("Reading pnpm-workspace.yaml...");
    let raw = match std::fs::read_to_string(&yaml_path) {
        Ok(content) => {
            task.complete("Read pnpm-workspace.yaml");
            content
        }
        Err(e) => {
            task.fail("Reading pnpm-workspace.yaml");
            return Err(anyhow::anyhow!(
                "failed to read {}: {e}",
                yaml_path.display()
            ));
        }
    };

    let task = runner.task("Parsing catalog entries...");
    let catalog = match riri_pnpm::catalog::PnpmCatalog::parse(&raw) {
        Ok(c) => {
            task.complete("Parsed catalog entries");
            c
        }
        Err(e) => {
            task.fail("Parsing catalog entries");
            return Err(anyhow::anyhow!(e));
        }
    };

    let pins = crate::pin_catalog_entries(&catalog, lockfile);
    Ok(CatalogPlan {
        pins,
        source: Some(CatalogSource {
            path: yaml_path,
            raw,
        }),
    })
}

/// Entry point shared by the standalone binary and the JS bin shim.
///
/// `argv` must include the program name at index 0 (mirroring `std::env::args`).
#[must_use]
pub fn run_cli(argv: Vec<String>) -> i32 {
    let parsed = match Args::try_parse_from(argv) {
        Ok(parsed) => parsed,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                EXIT_USAGE_ERROR
            } else {
                EXIT_OK
            };
        }
    };
    match run(&parsed) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{} {error:#}", style("error:").red().bold());
            EXIT_USAGE_ERROR
        }
    }
}
