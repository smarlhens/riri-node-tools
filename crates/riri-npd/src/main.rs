#![allow(clippy::missing_panics_doc)]

use anyhow::{Context, Result};
use clap::Parser;
use comfy_table::{Table, presets};
use console::style;
use riri_common::{LockfileVersions, PackageJsonFile, PackageManager, detect_lockfile};
use riri_npd::{DependencyKind, VersionToPin, pin_dependencies};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_task_runner::{RendererMode, TaskRunner};
use std::process::ExitCode;

/// Pin range-based dependency specifiers to the exact versions resolved by
/// the lockfile.
#[derive(Debug, Parser)]
#[command(name = "npd", version, about)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Silent mode — no output.
    #[arg(short, long)]
    quiet: bool,

    /// Verbose output.
    #[arg(short, long)]
    verbose: bool,

    /// Debug mode — detailed logging.
    #[arg(short, long)]
    debug: bool,

    /// Update package.json with pinned versions.
    #[arg(short, long)]
    update: bool,

    /// Output results as JSON.
    #[arg(long)]
    json: bool,

    /// Sort package.json keys on write (uses sort-package-json conventions).
    #[arg(long)]
    sort: bool,
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

fn parse_lockfile(manager: &PackageManager, content: &str) -> Result<Box<dyn LockfileVersions>> {
    match manager {
        PackageManager::Npm => {
            let lock =
                NpmPackageLock::parse(content).context("failed to parse package-lock.json")?;
            Ok(Box::new(lock))
        }
        PackageManager::Pnpm => {
            let lock = PnpmLockfile::parse(content).context("failed to parse pnpm-lock.yaml")?;
            Ok(Box::new(lock))
        }
        PackageManager::Yarn => {
            anyhow::bail!("yarn support is not yet wired into npd; track it in Phase 9.x")
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<ExitCode> {
    let mode = renderer_mode(args);
    let runner = TaskRunner::new(mode);
    let cwd = std::env::current_dir().context("failed to get current directory")?;

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
    let lockfile_content =
        std::fs::read_to_string(&lockfile_result.path).context("failed to read lockfile")?;
    let lockfile = match parse_lockfile(&lockfile_result.package_manager, &lockfile_content) {
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
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        });
    }

    if pins.is_empty() {
        if !args.quiet {
            eprintln!(
                "\n  All dependencies are already pinned {}",
                style(":)").green()
            );
        }
        return Ok(ExitCode::SUCCESS);
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
            eprintln!("  {}", line.trim());
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
    } else if !args.quiet {
        let hint = generate_update_hint(args);
        eprintln!(
            "\n  Run {} to upgrade package.json.",
            style(hint).bold().cyan()
        );
    }

    Ok(ExitCode::from(1))
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

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{} {e:#}", style("error:").red().bold());
            ExitCode::from(2)
        }
    }
}
