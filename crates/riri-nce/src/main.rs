#![allow(clippy::missing_panics_doc)]

use anyhow::{Context, Result};
use clap::Parser;
use comfy_table::{Table, presets};
use console::style;
use riri_common::{
    EngineConstraintKey, LockfileEngines, PackageJsonFile, PackageManager, detect_lockfile,
};
use riri_nce::{CheckEnginesInput, apply_engines_to_lockfile, apply_engines_update, check_engines};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_task_runner::{RendererMode, TaskRunner};
use riri_yarn::YarnProject;
use std::process::ExitCode;

/// Check and update Node.js engine constraints in package.json
/// based on the dependency tree from the lockfile.
#[derive(Debug, Parser)]
#[command(name = "nce", version, about)]
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

    /// Engine keys to check (e.g. node, npm, yarn). Defaults to all.
    #[arg(short, long, value_delimiter = ',')]
    engines: Vec<String>,

    /// Update package.json (and lockfile) with computed ranges.
    #[arg(short, long)]
    update: bool,

    /// Create or update .npmrc with engine-strict=true.
    #[arg(long)]
    enable_engine_strict: bool,

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

fn parse_engine_filters(raw: &[String]) -> Vec<EngineConstraintKey> {
    raw.iter()
        .filter_map(|s| EngineConstraintKey::from_str_lowercase(s))
        .collect()
}

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<ExitCode> {
    let mode = renderer_mode(args);
    let runner = TaskRunner::new(mode);
    let cwd = std::env::current_dir().context("failed to get current directory")?;

    // Detect lockfile
    let task = runner.task("Detecting lockfile...");
    match detect_lockfile(&cwd) {
        Err(e) => {
            task.fail("Detecting lockfile");
            return Err(anyhow::anyhow!(e));
        }
        Ok(ref result) => {
            task.complete(&format!(
                "Detected {}",
                result
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }
    }
    let lockfile_result = detect_lockfile(&cwd)?;

    // Read package.json
    let task = runner.task("Reading package.json...");
    let pkg_path = cwd.join("package.json");
    match PackageJsonFile::read(&pkg_path) {
        Err(e) => {
            task.fail("Reading package.json");
            return Err(anyhow::anyhow!(e));
        }
        Ok(_) => {
            task.complete("Read package.json");
        }
    }
    let mut pkg_file = PackageJsonFile::read(&pkg_path)?;

    // Parse lockfile
    let task = runner.task("Parsing lockfile...");
    let lockfile_content =
        std::fs::read_to_string(&lockfile_result.path).context("failed to read lockfile")?;

    let parsed_lock: Box<dyn LockfileEngines> = match lockfile_result.package_manager {
        PackageManager::Npm => match NpmPackageLock::parse(&lockfile_content) {
            Err(e) => {
                task.fail("Parsing lockfile");
                return Err(anyhow::anyhow!(e));
            }
            Ok(lock) => {
                task.complete("Parsed lockfile");
                Box::new(lock)
            }
        },
        PackageManager::Pnpm => match PnpmLockfile::parse(&lockfile_content) {
            Err(e) => {
                task.fail("Parsing lockfile");
                return Err(anyhow::anyhow!(e));
            }
            Ok(lock) => {
                task.complete("Parsed lockfile");
                Box::new(lock)
            }
        },
        PackageManager::Yarn => {
            let project_dir = lockfile_result
                .path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            match YarnProject::scan(project_dir) {
                Err(e) => {
                    task.fail("Scanning node_modules");
                    return Err(anyhow::anyhow!(e));
                }
                Ok(project) => {
                    task.complete("Scanned node_modules");
                    Box::new(project)
                }
            }
        }
    };

    // Compute engine constraints
    let task = runner.task("Computing engine constraints...");
    let entries: Vec<(&str, &riri_common::Engines)> = parsed_lock.engines_iter().collect();
    let filter_engines = parse_engine_filters(&args.engines);

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg_file.parsed.engines.as_ref(),
        filter_engines,
    };
    let output = check_engines(&input);
    task.complete("Computed engine constraints");

    // JSON output mode
    if args.json {
        let json_output = serde_json::json!({
            "computed": output.computed_engines.iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect::<std::collections::BTreeMap<String, String>>(),
            "changes": output.engines_range_to_set.iter()
                .map(|c| serde_json::json!({
                    "engine": c.engine.to_string(),
                    "from": c.range,
                    "to": c.range_to_set,
                }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return if output.engines_range_to_set.is_empty() {
            Ok(ExitCode::SUCCESS)
        } else {
            Ok(ExitCode::from(1))
        };
    }

    // Display results
    if output.engines_range_to_set.is_empty() {
        if !args.quiet {
            eprintln!(
                "\n  All computed engines range constraints are up-to-date {}",
                style(":)").green()
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    if !args.quiet {
        let mut table = Table::new();
        table.load_preset(presets::NOTHING);
        for change in &output.engines_range_to_set {
            table.add_row(vec![
                change.engine.to_string(),
                change.range.clone(),
                "\u{2192}".to_string(),
                change.range_to_set.clone(),
            ]);
        }

        for line in table.lines() {
            eprintln!("  {}", line.trim());
        }
    }

    // Update if requested
    if args.update {
        let task = runner.task("Updating package.json...");
        apply_engines_update(&mut pkg_file, &output.engines_range_to_set);
        if args.sort {
            pkg_file
                .write_sorted()
                .context("failed to write package.json")?;
        } else {
            pkg_file.write().context("failed to write package.json")?;
        }
        task.complete("Updated package.json");

        if lockfile_result.package_manager == PackageManager::Npm {
            let task = runner.task("Updating lockfile...");
            let mut lockfile_raw: serde_json::Value = serde_json::from_str(&lockfile_content)?;
            apply_engines_to_lockfile(&mut lockfile_raw, &output.engines_range_to_set);
            let lockfile_out = serde_json::to_string_pretty(&lockfile_raw)? + "\n";
            std::fs::write(&lockfile_result.path, lockfile_out)
                .context("failed to write lockfile")?;
            task.complete("Updated lockfile");
        }
    } else if !args.quiet {
        let hint = generate_update_hint(args);
        eprintln!(
            "\n  Run {} to upgrade package.json.",
            style(hint).bold().cyan()
        );
    }

    // Enable engine-strict in .npmrc
    if args.enable_engine_strict {
        let task = runner.task("Enabling engine-strict in .npmrc...");
        let npmrc_path = cwd.join(".npmrc");
        let content = std::fs::read_to_string(&npmrc_path).unwrap_or_default();
        if content.contains("engine-strict=true") {
            task.skip("Enabling engine-strict in .npmrc", "already enabled");
        } else {
            let new_content = if content.is_empty() {
                "engine-strict=true\n".to_string()
            } else {
                format!("{content}engine-strict=true\n")
            };
            std::fs::write(&npmrc_path, new_content).context("failed to write .npmrc")?;
            task.complete("Enabled engine-strict in .npmrc");
        }
    }

    Ok(ExitCode::from(1))
}

fn generate_update_hint(args: &Args) -> String {
    let mut parts = vec!["nce".to_string()];
    if args.quiet {
        parts.push("-q".to_string());
    }
    if args.verbose {
        parts.push("-v".to_string());
    }
    if args.debug {
        parts.push("-d".to_string());
    }
    if !args.engines.is_empty() {
        parts.push(format!("-e {}", args.engines.join(",")));
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
