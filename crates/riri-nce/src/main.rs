#![allow(clippy::missing_panics_doc)]

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use clap::Parser;
use comfy_table::{Table, presets};
use console::style;
use riri_common::{
    EngineConstraintKey, LockfileEngines, PackageJsonFile, PackageManager, detect_lockfile,
};
use riri_nce::{
    CheckEnginesInput, LifecycleConfig, LifecycleOutput, apply_engines_to_lockfile,
    apply_engines_update, check_engines_with_lifecycle,
};
use riri_node_lifecycle::{LifecycleData, Policy};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_semver_range::VersionPrecision;
use riri_task_runner::{RendererMode, TaskRunner};
use riri_yarn::YarnProject;
use std::path::PathBuf;
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

    /// Version precision in output: major (e.g. >=24), minor (e.g. >=24.0),
    /// or patch (e.g. >=24.0.0). Trailing .0 components are trimmed accordingly.
    /// Non-zero components are never dropped.
    #[arg(long, value_enum, default_value_t = PrecisionArg::Patch)]
    precision: PrecisionArg,

    /// Node.js lifecycle policy gate for engines.node.
    #[arg(long, value_enum, default_value_t = NodePolicyArg::Supported)]
    node_policy: NodePolicyArg,

    /// Suppress EOL warnings (does not widen the policy).
    #[arg(long)]
    allow_eol: bool,

    /// Bump engines.npm floor to match the lowest node major in range.
    #[arg(long, overrides_with = "no_bump_npm")]
    bump_npm: bool,

    /// Disable the npm coupling pass.
    #[arg(long, overrides_with = "bump_npm")]
    no_bump_npm: bool,

    /// Precision applied to the npm bump floor.
    #[arg(long, value_enum, default_value_t = PrecisionArg::Major)]
    npm_precision: PrecisionArg,

    /// Override path to lifecycle data JSON (test/CI hook).
    #[arg(long, hide = true)]
    node_data: Option<PathBuf>,

    /// Override "today" as YYYY-MM-DD (test hook).
    #[arg(long, hide = true)]
    today: Option<String>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PrecisionArg {
    /// Trim all trailing .0 (minimum 1 component).
    Major,
    /// Trim trailing .0 patch only (minimum 2 components).
    Minor,
    /// Always show major.minor.patch.
    Patch,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum NodePolicyArg {
    Any,
    Stable,
    Supported,
    Lts,
    Maintenance,
}

fn to_version_precision(arg: PrecisionArg) -> VersionPrecision {
    match arg {
        PrecisionArg::Major => VersionPrecision::Major,
        PrecisionArg::Minor => VersionPrecision::MajorMinor,
        PrecisionArg::Patch => VersionPrecision::Full,
    }
}

fn to_policy(arg: NodePolicyArg) -> Policy {
    match arg {
        NodePolicyArg::Any => Policy::Any,
        NodePolicyArg::Stable => Policy::Stable,
        NodePolicyArg::Supported => Policy::Supported,
        NodePolicyArg::Lts => Policy::Lts,
        NodePolicyArg::Maintenance => Policy::Maintenance,
    }
}

fn policy_label(p: Policy) -> &'static str {
    match p {
        Policy::Any => "any",
        Policy::Stable => "stable",
        Policy::Supported => "supported",
        Policy::Lts => "lts",
        Policy::Maintenance => "maintenance",
    }
}

fn bump_npm_enabled(args: &Args) -> bool {
    !args.no_bump_npm
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
        precision: to_version_precision(args.precision),
    };

    let today = resolve_today(args)?;
    let lifecycle_data = load_lifecycle_data(args, today)?;
    let cfg = LifecycleConfig {
        data: &lifecycle_data,
        policy: to_policy(args.node_policy),
        today,
        allow_eol: args.allow_eol,
        bump_npm: bump_npm_enabled(args),
        npm_precision: to_version_precision(args.npm_precision),
    };
    let (output, lifecycle) = check_engines_with_lifecycle(&input, &cfg)
        .map_err(|e| anyhow::anyhow!("lifecycle rewrite failed: {e}"))?;
    task.complete("Computed engine constraints");

    if let Some(code) = check_data_freshness(args, &lifecycle_data, today) {
        return Ok(code);
    }

    emit_eol_warnings(args, &lifecycle);

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
            "lifecycle": lifecycle_json(&lifecycle),
            "npm_bump": npm_bump_json(lifecycle.npm_bump.as_ref()),
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(exit_code_for(&output, &lifecycle));
    }

    if lifecycle.unsatisfiable {
        emit_unsatisfiable(args, &lifecycle);
        return Ok(ExitCode::from(3));
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

const DEFAULT_MAX_DATA_AGE_DAYS: i64 = 180;

fn max_data_age_days() -> i64 {
    std::env::var("RIRI_NCE_MAX_DATA_AGE_DAYS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n >= 0)
        .unwrap_or(DEFAULT_MAX_DATA_AGE_DAYS)
}

fn data_age_days(fetched_at: chrono::DateTime<Utc>, today: NaiveDate) -> i64 {
    today
        .signed_duration_since(fetched_at.date_naive())
        .num_days()
}

fn check_data_freshness(args: &Args, data: &LifecycleData, today: NaiveDate) -> Option<ExitCode> {
    let age = data_age_days(data.fetched_at, today);
    let max = max_data_age_days();
    let warn_threshold = max / 2;
    if age > max && args.update {
        if !args.quiet {
            eprintln!(
                "\n  {} lifecycle data is {age} days old (max {max}). Run `nce --refresh` to update.",
                style("error:").red().bold(),
            );
        }
        return Some(ExitCode::from(1));
    }
    if age > warn_threshold && !args.quiet {
        eprintln!(
            "  {} lifecycle data is {age} days old. Consider running `nce --refresh`.",
            style("warning:").yellow().bold(),
        );
    }
    None
}

fn load_lifecycle_data(args: &Args, today: NaiveDate) -> Result<LifecycleData> {
    let mut data = if let Some(path) = &args.node_data {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        LifecycleData::parse(&raw).with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        LifecycleData::bundled().clone()
    };
    data.resolve_statuses(today);
    Ok(data)
}

fn resolve_today(args: &Args) -> Result<NaiveDate> {
    if let Some(s) = &args.today {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .with_context(|| format!("--today must be YYYY-MM-DD, got {s}"))
    } else {
        Ok(Utc::now().date_naive())
    }
}

fn lifecycle_json(lifecycle: &LifecycleOutput) -> serde_json::Value {
    serde_json::json!({
        "policy": lifecycle.policy.map(policy_label),
        "data_fetched_at": lifecycle.data_fetched_at,
        "warnings": lifecycle.warnings.iter().map(|w| serde_json::json!({
            "kind": "eol",
            "engine": "node",
            "major": w.major,
            "since": w.since,
        })).collect::<Vec<_>>(),
        "dropped_disjuncts": lifecycle.dropped_disjuncts,
        "bumped_disjuncts": lifecycle.bumped_disjuncts.iter().map(|(from, to)| serde_json::json!({
            "from": from,
            "to": to,
        })).collect::<Vec<_>>(),
        "unsatisfiable": lifecycle.unsatisfiable,
    })
}

fn npm_bump_json(bump: Option<&riri_nce::NpmBumpResult>) -> serde_json::Value {
    bump.map_or(serde_json::Value::Null, |b| {
        serde_json::json!({
            "target": b.target_floor.to_string(),
            "applied": b.apply,
            "reason": b.reason,
        })
    })
}

fn exit_code_for(output: &riri_nce::CheckEnginesOutput, lifecycle: &LifecycleOutput) -> ExitCode {
    if lifecycle.unsatisfiable {
        ExitCode::from(3)
    } else if output.engines_range_to_set.is_empty() && lifecycle.warnings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn emit_unsatisfiable(args: &Args, lifecycle: &LifecycleOutput) {
    if args.quiet {
        return;
    }
    eprintln!(
        "\n  {} --node-policy={} unsatisfiable",
        style("error:").red().bold(),
        policy_label(lifecycle.policy.unwrap_or(Policy::Supported))
    );
    if !lifecycle.dropped_disjuncts.is_empty() {
        eprintln!("  dropped: {}", lifecycle.dropped_disjuncts.join(", "));
    }
}

fn emit_eol_warnings(args: &Args, lifecycle: &LifecycleOutput) {
    if args.quiet || args.allow_eol {
        return;
    }
    for w in &lifecycle.warnings {
        eprintln!(
            "  {} node {} reached end-of-life on {}",
            style("warning:").yellow().bold(),
            w.major,
            w.since
        );
    }
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
