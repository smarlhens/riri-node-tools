//! CLI logic for `ncd` — exposed both via the `riri-ncd` binaries and the
//! `riri-napi-ncd` JS shim.

use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use riri_common::{
    LockGraph, LockfileGraph, NpmrcRegistryConfig, PackageJson, PackageManager, detect_lockfile,
    find_package_json,
};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_task_runner::{RendererMode, TaskRunner};
use riri_workspace::{WorkspaceMember, WorkspaceProject};
use riri_yarn::YarnLock;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::analyze::{Report, analyze};
use crate::registry::RegistryClient;
use crate::render::render_tree;
use crate::{DeprecationSource, Packument, SourceError};

const EXIT_OK: i32 = 0;
const EXIT_DEPRECATED_FOUND: i32 = 1;
const EXIT_ERROR: i32 = 2;

/// Find deprecated packages anywhere in the lockfile dependency tree.
#[derive(Debug, Parser)]
#[command(name = "ncd", version, about)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Silent mode — no progress output.
    #[arg(short, long)]
    pub quiet: bool,

    /// Verbose output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Debug mode — detailed logging.
    #[arg(short, long)]
    pub debug: bool,

    /// Output results as JSON.
    #[arg(long)]
    pub json: bool,

    /// Registry URL override (default: .npmrc, then <https://registry.npmjs.org>).
    #[arg(long)]
    pub registry: Option<String>,
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

/// Parse the detected lockfile and extract its full dependency graph.
fn load_graph(
    manager: &PackageManager,
    lockfile_path: &Path,
    package_json: &PackageJson,
) -> Result<LockGraph> {
    let content = std::fs::read_to_string(lockfile_path)
        .with_context(|| format!("failed to read {}", lockfile_path.display()))?;
    let graph = match manager {
        PackageManager::Npm => NpmPackageLock::parse(&content)
            .context("failed to parse package-lock.json")?
            .dep_graph(package_json),
        PackageManager::Pnpm => PnpmLockfile::parse(&content)
            .context("failed to parse pnpm-lock.yaml")?
            .dep_graph(package_json),
        PackageManager::Yarn => YarnLock::parse(&content)
            .context("failed to parse yarn.lock")?
            .dep_graph(package_json),
    };
    graph.map_err(|e| anyhow::anyhow!("failed to build dependency graph: {e}"))
}

fn project_name(package_json: &PackageJson, cwd: &Path) -> String {
    package_json.name.clone().unwrap_or_else(|| {
        cwd.file_name().map_or_else(
            || "project".to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    })
}

fn unique_package_count(graph: &LockGraph) -> usize {
    graph
        .nodes
        .iter()
        .map(|n| n.name.as_str())
        .collect::<HashSet<_>>()
        .len()
}

/// Testable core: detect, parse, analyze, render. `source` supplies packuments.
///
/// # Errors
/// Propagates lockfile/`package.json`/registry failures as `anyhow` errors.
pub fn run_with_source(args: &Args, cwd: &Path, source: &dyn DeprecationSource) -> Result<i32> {
    let mode = renderer_mode(args);
    if args.debug {
        install_debug_subscriber();
    }
    let runner = TaskRunner::new(mode);

    let task = runner.task("Detecting lockfile...");
    let lockfile_result = match detect_lockfile(cwd) {
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
    let (package_json, _) = match find_package_json(cwd) {
        Ok(found) => {
            task.complete("Read package.json");
            found
        }
        Err(e) => {
            task.fail("Reading package.json");
            return Err(anyhow::anyhow!(e));
        }
    };

    let task = runner.task("Building dependency graph...");
    let graph = match load_graph(
        &lockfile_result.package_manager,
        &lockfile_result.path,
        &package_json,
    ) {
        Ok(graph) => {
            task.complete("Built dependency graph");
            graph
        }
        Err(e) => {
            task.fail("Building dependency graph");
            return Err(e);
        }
    };

    let name = project_name(&package_json, cwd);
    let count = unique_package_count(&graph);

    let task = runner.task(&format!("Checking {count} packages against registry..."));
    let report = match analyze(&graph, &name, source) {
        Ok(report) => {
            task.complete("Checked packages against registry");
            report
        }
        Err(e) => {
            task.fail("Checking packages against registry");
            return Err(anyhow::anyhow!(e));
        }
    };

    // Human output goes to stderr; stdout is reserved for `--json` (matches nce/npd).
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if let Some(tree) = &report.tree {
        eprintln!("{}", render_tree(tree));
    } else if !args.quiet {
        eprintln!("\n  No deprecated packages found {}", style(":)").green());
    }

    if !args.json && !args.quiet && !report.deprecated.is_empty() {
        eprintln!(
            "\n  {} deprecated package(s) found",
            style(report.deprecated.len()).bold().yellow()
        );
    }

    Ok(if report.deprecated.is_empty() {
        EXIT_OK
    } else {
        EXIT_DEPRECATED_FOUND
    })
}

fn install_debug_subscriber() {
    use std::io::IsTerminal;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("riri_ncd=debug"));
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init();
}

/// In-memory [`DeprecationSource`] backed by packuments prefetched once for the
/// whole workspace, so per-member analysis performs no further network calls.
struct MapSource(HashMap<String, Packument>);

impl DeprecationSource for MapSource {
    fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
        Ok(self.0.get(name).cloned())
    }
}

/// Parse the detected lockfile once into a graph-capable handle reused across
/// every workspace member.
fn parse_lockfile_graph(
    manager: &PackageManager,
    lockfile_path: &Path,
) -> Result<Box<dyn LockfileGraph>> {
    let content = std::fs::read_to_string(lockfile_path)
        .with_context(|| format!("failed to read {}", lockfile_path.display()))?;
    Ok(match manager {
        PackageManager::Npm => {
            Box::new(NpmPackageLock::parse(&content).context("failed to parse package-lock.json")?)
        }
        PackageManager::Pnpm => {
            Box::new(PnpmLockfile::parse(&content).context("failed to parse pnpm-lock.yaml")?)
        }
        PackageManager::Yarn => {
            Box::new(YarnLock::parse(&content).context("failed to parse yarn.lock")?)
        }
    })
}

fn read_member_pkg(member: &WorkspaceMember) -> Result<PackageJson> {
    let content = std::fs::read_to_string(&member.manifest_path)
        .with_context(|| format!("failed to read {}", member.manifest_path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", member.manifest_path.display()))
}

/// Build each member's report against the shared root lockfile, fetching every
/// referenced packument once. Members are returned in enumeration order.
fn analyze_workspace(
    project: &WorkspaceProject,
    runner: &TaskRunner,
    source: &dyn DeprecationSource,
) -> Result<Vec<(WorkspaceMember, Report)>> {
    let members = project
        .members()
        .map_err(|e| anyhow::anyhow!("failed to enumerate workspace members: {e}"))?;
    if members.is_empty() {
        return Ok(Vec::new());
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
    let lockfile =
        match parse_lockfile_graph(&lockfile_result.package_manager, &lockfile_result.path) {
            Ok(lock) => {
                task.complete("Parsed lockfile");
                lock
            }
            Err(e) => {
                task.fail("Parsing lockfile");
                return Err(e);
            }
        };

    let task = runner.task("Building dependency graphs...");
    let mut member_graphs: Vec<(WorkspaceMember, LockGraph)> = Vec::new();
    for member in members {
        let pkg = read_member_pkg(&member).map_err(|e| anyhow::anyhow!("{}: {e}", member.name))?;
        let graph = lockfile.dep_graph(&pkg).map_err(|e| {
            anyhow::anyhow!("{}: failed to build dependency graph: {e}", member.name)
        })?;
        member_graphs.push((member, graph));
    }
    task.complete("Built dependency graphs");

    let mut names: Vec<String> = member_graphs
        .iter()
        .flat_map(|(_, g)| g.nodes.iter().map(|n| n.name.clone()))
        .collect();
    names.sort();
    names.dedup();

    let task = runner.task(&format!(
        "Checking {} packages against registry...",
        names.len()
    ));
    let (packuments, errors) = crate::registry::fetch_all(source, &names);
    if let Some(e) = errors.into_iter().next() {
        task.fail("Checking packages against registry");
        return Err(anyhow::anyhow!(e));
    }
    task.complete("Checked packages against registry");
    let mem_source = MapSource(packuments);

    let mut out = Vec::with_capacity(member_graphs.len());
    for (member, graph) in member_graphs {
        let report = analyze(&graph, &member.name, &mem_source).map_err(|e| anyhow::anyhow!(e))?;
        out.push((member, report));
    }
    Ok(out)
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

fn emit_workspace_text(args: &Args, per_member: &[(WorkspaceMember, Report)]) {
    if args.quiet {
        return;
    }
    let mut shown = 0_usize;
    let mut total = 0_usize;
    for (member, report) in per_member {
        let Some(tree) = &report.tree else { continue };
        if report.deprecated.is_empty() {
            continue;
        }
        shown += 1;
        total += report.deprecated.len();
        eprintln!("{}", style(format!("{}:", member.name)).bold());
        eprintln!("{}", render_tree(tree));
    }
    if shown == 0 {
        eprintln!("\n  No deprecated packages found {}", style(":)").green());
    } else {
        eprintln!(
            "\n  {} deprecated package(s) across {} workspace member(s)",
            style(total).bold().yellow(),
            shown,
        );
    }
}

fn emit_workspace_json(root: &Path, per_member: &[(WorkspaceMember, Report)]) {
    let members: Vec<serde_json::Value> = per_member
        .iter()
        .map(|(member, report)| {
            serde_json::json!({
                "name": member.name,
                "manifest": relative_manifest(root, &member.manifest_path),
                "tree": report.tree,
                "deprecated": report.deprecated,
            })
        })
        .collect();
    let out = serde_json::json!({ "members": members });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

/// Workspace entry point: analyze each member against the shared root lockfile
/// and group output per member. `source` supplies packuments.
///
/// # Errors
/// Propagates lockfile/`package.json`/registry failures as `anyhow` errors.
pub fn run_workspace_with_source(
    args: &Args,
    project: &WorkspaceProject,
    source: &dyn DeprecationSource,
) -> Result<i32> {
    if args.debug {
        install_debug_subscriber();
    }
    let runner = TaskRunner::new(renderer_mode(args));
    let per_member = analyze_workspace(project, &runner, source)?;

    if per_member.is_empty() {
        if !args.quiet {
            eprintln!("\n  No workspace members found");
        }
        return Ok(EXIT_OK);
    }

    if args.json {
        emit_workspace_json(project.root(), &per_member);
    } else {
        emit_workspace_text(args, &per_member);
    }

    Ok(
        if per_member.iter().any(|(_, r)| !r.deprecated.is_empty()) {
            EXIT_DEPRECATED_FOUND
        } else {
            EXIT_OK
        },
    )
}

/// Entry point shared by the standalone binaries and the JS bin shim.
///
/// `argv` must include the program name at index 0 (mirroring `std::env::args`).
#[must_use]
pub fn run_cli(argv: Vec<String>) -> i32 {
    let parsed = match Args::try_parse_from(argv) {
        Ok(parsed) => parsed,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                EXIT_ERROR
            } else {
                EXIT_OK
            };
        }
    };

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("{} {e}", style("error:").red().bold());
            return EXIT_ERROR;
        }
    };

    let config = NpmrcRegistryConfig::read(&cwd, None);
    let client = RegistryClient::new(config, parsed.registry.clone());

    let result = match riri_workspace::detect(&cwd) {
        Some(project) => run_workspace_with_source(&parsed, &project, &client),
        None => run_with_source(&parsed, &cwd, &client),
    };

    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{} {error:#}", style("error:").red().bold());
            EXIT_ERROR
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Packument, SourceError};
    use std::collections::HashMap;

    struct StubSource(HashMap<String, Packument>);
    impl DeprecationSource for StubSource {
        fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
            Ok(self.0.get(name).cloned())
        }
    }
    fn stub(map: &[(&str, &str)]) -> StubSource {
        StubSource(
            map.iter()
                .map(|(n, j)| ((*n).to_string(), serde_json::from_str(j).unwrap()))
                .collect(),
        )
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn args() -> Args {
        Args {
            quiet: true,
            verbose: false,
            debug: false,
            json: true,
            registry: None,
        }
    }

    #[test]
    fn deprecated_transitive_exits_one() {
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "2.1.0"}, "versions": {"1.0.0": {"deprecated": "use @foo/core instead"}, "2.1.0": {}}}"#,
            ),
        ]);
        let code = run_with_source(
            &args(),
            &fixture("ncd-npm-v3-deprecated-transitive"),
            &source,
        )
        .unwrap();
        assert_eq!(code, EXIT_DEPRECATED_FOUND);
    }

    #[test]
    fn nothing_deprecated_exits_zero() {
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "1.0.0"}, "versions": {"1.0.0": {}}}"#,
            ),
        ]);
        let code = run_with_source(
            &args(),
            &fixture("ncd-npm-v3-deprecated-transitive"),
            &source,
        )
        .unwrap();
        assert_eq!(code, EXIT_OK);
    }

    /// Stable core fields of a finding: `(name, version, message, latest, deps)`.
    type CoreFinding = (String, String, Option<String>, Option<String>, Vec<String>);

    /// Build the report for a fixture directory (detect → graph → analyze).
    fn report_for(dir: &str, source: &dyn DeprecationSource) -> crate::analyze::Report {
        let path = fixture(dir);
        let result = detect_lockfile(&path).unwrap();
        let (pkg, _) = find_package_json(&path).unwrap();
        let graph = load_graph(&result.package_manager, &result.path, &pkg).unwrap();
        analyze(&graph, "ncd-fixture", source).unwrap()
    }

    #[test]
    fn deprecated_findings_identical_across_package_managers() {
        let fixtures = [
            "ncd-npm-v3-deprecated-transitive",
            "ncd-yarn-berry-deprecated-transitive",
            "ncd-pnpm-v9-deprecated-transitive",
        ];
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "2.1.0"}, "versions": {"1.0.0": {"deprecated": "use @foo/core instead"}, "2.1.0": {}}}"#,
            ),
        ]);
        // Core fields of the single deprecated finding, per package manager.
        let core: Vec<CoreFinding> = fixtures
            .iter()
            .map(|dir| {
                let report = report_for(dir, &source);
                assert_eq!(report.deprecated.len(), 1, "{dir}");
                let f = &report.deprecated[0];
                (
                    f.name.clone(),
                    f.version.clone(),
                    f.message.clone(),
                    f.latest.clone(),
                    f.direct_dependents.clone(),
                )
            })
            .collect();
        assert_eq!(
            core[0],
            (
                "foo".to_string(),
                "1.0.0".to_string(),
                Some("use @foo/core instead".to_string()),
                Some("2.1.0".to_string()),
                vec!["bar".to_string()],
            )
        );
        // All three package managers produce identical core findings.
        assert_eq!(core[0], core[1]);
        assert_eq!(core[0], core[2]);
    }

    #[test]
    fn yarn_and_pnpm_fixtures_exit_one() {
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "2.1.0"}, "versions": {"1.0.0": {"deprecated": "use @foo/core instead"}, "2.1.0": {}}}"#,
            ),
        ]);
        for dir in [
            "ncd-yarn-berry-deprecated-transitive",
            "ncd-pnpm-v9-deprecated-transitive",
        ] {
            let code = run_with_source(&args(), &fixture(dir), &source).unwrap();
            assert_eq!(code, EXIT_DEPRECATED_FOUND, "{dir}");
        }
    }

    fn workspace_stub() -> StubSource {
        stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "2.1.0"}, "versions": {"1.0.0": {"deprecated": "use @foo/core instead"}, "2.1.0": {}}}"#,
            ),
            (
                "qux",
                r#"{"dist-tags": {"latest": "1.0.0"}, "versions": {"1.0.0": {}}}"#,
            ),
        ])
    }

    #[test]
    fn workspace_groups_findings_per_member() {
        let source = workspace_stub();
        let project =
            riri_workspace::detect(&fixture("ncd-npm-workspace")).expect("workspace detected");
        let runner = TaskRunner::new(RendererMode::Silent);
        let per_member = analyze_workspace(&project, &runner, &source).unwrap();
        assert_eq!(per_member.len(), 2);

        let by_name: HashMap<&str, &Report> = per_member
            .iter()
            .map(|(m, r)| (m.name.as_str(), r))
            .collect();
        // @ncd/a → bar → foo (deprecated); @ncd/b → qux (clean).
        let a = by_name["@ncd/a"];
        assert_eq!(a.deprecated.len(), 1);
        assert_eq!(a.deprecated[0].name, "foo");
        assert!(a.tree.is_some());
        let b = by_name["@ncd/b"];
        assert!(b.deprecated.is_empty());
        assert!(b.tree.is_none());
    }

    #[test]
    fn workspace_exits_one_when_any_member_deprecated() {
        let source = workspace_stub();
        let project = riri_workspace::detect(&fixture("ncd-npm-workspace")).unwrap();
        let code = run_workspace_with_source(&args(), &project, &source).unwrap();
        assert_eq!(code, EXIT_DEPRECATED_FOUND);
    }
}
