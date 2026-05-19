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
use std::path::Path;

use crate::{DependencyKind, VersionToPin, pin_dependencies};

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

    if args.json {
        let json_output = serde_json::json!({
            "pins": pins.iter().map(|p| serde_json::json!({
                "name": p.name,
                "kind": p.kind.as_str(),
                "from": p.current_range,
                "to": p.pinned_version,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(if pins.is_empty() {
            EXIT_OK
        } else {
            EXIT_PINS_PENDING
        });
    }

    if pins.is_empty() {
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
            ]);
        }
        for line in table.lines() {
            eprintln!("    {}", line.trim());
        }
    }

    if args.update {
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
                "\n  Run {} to upgrade package.json.",
                style(hint).bold().cyan()
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
    parts.join(" ")
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
