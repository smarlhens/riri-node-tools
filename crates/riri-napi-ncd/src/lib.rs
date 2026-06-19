#![deny(clippy::all)]
// NAPI exports are consumed by the C ABI, not Rust — dead_code is a false positive for cdylib
#![allow(dead_code)]
// NAPI #[napi] functions require owned String parameters, not &str
#![allow(clippy::needless_pass_by_value)]
// Errors surface to JS via napi::Error::from_reason; documenting each is noise.
#![allow(clippy::missing_errors_doc)]

use napi_derive::napi;
use riri_ncd::analyze::{Blocker, DeprecatedFinding, Report};
use riri_ncd::{check_deprecations_from_content, render::render_tree};

/// A package whose declared range blocks the fix for a deprecated dependency.
#[napi(object)]
pub struct DeprecationBlocker {
    pub name: String,
    pub version: String,
    /// Range the blocker declares for the deprecated package.
    pub requires: String,
    /// Newest non-deprecated version the fix needs.
    pub fix_needs: String,
}

/// One deprecated package instance and how (if at all) it can be fixed.
#[napi(object)]
pub struct DeprecatedPackage {
    pub name: String,
    pub version: String,
    pub message: Option<String>,
    pub latest: Option<String>,
    pub update_fixable: bool,
    pub fix_version: Option<String>,
    pub needs_replacement: bool,
    pub blockers: Vec<DeprecationBlocker>,
    pub direct_dependents: Vec<String>,
}

#[napi(object)]
pub struct CheckDeprecationsResult {
    /// Rendered dependency tree (the text the CLI prints), present only when at
    /// least one deprecated package was found.
    pub tree: Option<String>,
    pub deprecated: Vec<DeprecatedPackage>,
}

#[napi(object)]
pub struct CheckDeprecationsOptions {
    pub package_json: String,
    pub lockfile_content: String,
    /// `"npm"` | `"yarn"` | `"pnpm"`. Defaults to `"npm"` when omitted.
    pub lockfile_type: Option<String>,
    /// Registry URL override. Defaults to <https://registry.npmjs.org>.
    pub registry: Option<String>,
}

fn map_blocker(blocker: Blocker) -> DeprecationBlocker {
    DeprecationBlocker {
        name: blocker.name,
        version: blocker.version,
        requires: blocker.requires,
        fix_needs: blocker.fix_needs,
    }
}

fn map_finding(finding: DeprecatedFinding) -> DeprecatedPackage {
    DeprecatedPackage {
        name: finding.name,
        version: finding.version,
        message: finding.message,
        latest: finding.latest,
        update_fixable: finding.update_fixable,
        fix_version: finding.fix_version,
        needs_replacement: finding.needs_replacement,
        blockers: finding.blockers.into_iter().map(map_blocker).collect(),
        direct_dependents: finding.direct_dependents,
    }
}

fn map_report(report: Report) -> CheckDeprecationsResult {
    CheckDeprecationsResult {
        tree: report.tree.as_ref().map(render_tree),
        deprecated: report.deprecated.into_iter().map(map_finding).collect(),
    }
}

/// Analyze a lockfile's dependency tree against the registry and return the
/// deprecated packages plus the rendered tree. Performs blocking network I/O.
#[napi]
pub fn check_deprecations(
    options: CheckDeprecationsOptions,
) -> napi::Result<CheckDeprecationsResult> {
    let report = check_deprecations_from_content(
        &options.package_json,
        &options.lockfile_content,
        options.lockfile_type.as_deref().unwrap_or("npm"),
        options.registry,
    )
    .map_err(|error| napi::Error::from_reason(format!("check_deprecations failed: {error:#}")))?;
    Ok(map_report(report))
}

/// Run the `ncd` CLI in-process. `argv` must include the program name at
/// index 0 (e.g. `["ncd", "--json"]`). Returns the exit code.
#[napi]
#[must_use]
pub fn run_cli(argv: Vec<String>) -> i32 {
    riri_ncd::cli::run_cli(argv)
}
