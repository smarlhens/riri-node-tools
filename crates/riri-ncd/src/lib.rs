//! Core logic for `npm-check-deprecations`.

pub mod analyze;
pub mod cli;
pub mod registry;
pub mod render;

use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;

/// Analyze a project's lockfile (supplied as in-memory content) against the
/// live npm registry and return the deprecation [`Report`](analyze::Report).
///
/// `lockfile_type` is `"npm"`, `"pnpm"`, or `"yarn"`. `registry` overrides the
/// registry URL (defaults to <https://registry.npmjs.org>). Performs blocking
/// network requests; `.npmrc` is not consulted (pass `registry` explicitly for
/// private registries).
///
/// # Errors
/// Parse failures, an unknown `lockfile_type`, or registry/auth failures.
pub fn check_deprecations_from_content(
    package_json: &str,
    lockfile_content: &str,
    lockfile_type: &str,
    registry: Option<String>,
) -> anyhow::Result<analyze::Report> {
    use anyhow::Context as _;
    use riri_common::{LockfileGraph as _, NpmrcRegistryConfig, PackageJson};

    let pkg: PackageJson =
        serde_json::from_str(package_json).context("failed to parse package.json")?;

    let graph = match lockfile_type {
        "npm" => riri_npm::NpmPackageLock::parse(lockfile_content)
            .context("failed to parse package-lock.json")?
            .dep_graph(&pkg),
        "pnpm" => riri_pnpm::PnpmLockfile::parse(lockfile_content)
            .context("failed to parse pnpm-lock.yaml")?
            .dep_graph(&pkg),
        "yarn" => riri_yarn::YarnLock::parse(lockfile_content)
            .context("failed to parse yarn.lock")?
            .dep_graph(&pkg),
        other => anyhow::bail!("unknown lockfile type: {other}"),
    }
    .map_err(|e| anyhow::anyhow!("failed to build dependency graph: {e}"))?;

    let project_name = pkg.name.clone().unwrap_or_else(|| "project".to_string());
    let client = registry::RegistryClient::new(NpmrcRegistryConfig::default(), registry);
    analyze::analyze(&graph, &project_name, &client).map_err(|error| anyhow::anyhow!(error))
}

/// `deprecated` is a string message in the wild, but some packuments carry a
/// boolean.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DeprecatedField {
    Message(String),
    Flag(bool),
}

/// One version entry of an abbreviated packument.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PackumentVersion {
    #[serde(default)]
    pub deprecated: Option<DeprecatedField>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, String>,
}

impl PackumentVersion {
    #[must_use]
    pub fn is_deprecated(&self) -> bool {
        match &self.deprecated {
            Some(DeprecatedField::Message(m)) => !m.is_empty(),
            Some(DeprecatedField::Flag(f)) => *f,
            None => false,
        }
    }

    /// Full deprecation message, when one exists (JSON output keeps it whole;
    /// the tree renderer shows only the first line).
    #[must_use]
    pub fn deprecation_message(&self) -> Option<&str> {
        match &self.deprecated {
            Some(DeprecatedField::Message(m)) if !m.is_empty() => Some(m),
            _ => None,
        }
    }

    /// Declared range for `name` across runtime + optional deps.
    #[must_use]
    pub fn declared_range(&self, name: &str) -> Option<&str> {
        self.dependencies
            .get(name)
            .or_else(|| self.optional_dependencies.get(name))
            .map(String::as_str)
    }
}

/// Abbreviated packument (`application/vnd.npm.install-v1+json`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Packument {
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    #[serde(default)]
    pub versions: HashMap<String, PackumentVersion>,
}

impl Packument {
    #[must_use]
    pub fn latest(&self) -> Option<&str> {
        self.dist_tags.get("latest").map(String::as_str)
    }

    /// Newest non-deprecated, non-prerelease version.
    #[must_use]
    pub fn newest_non_deprecated(&self) -> Option<Version> {
        self.versions
            .iter()
            .filter(|(_, v)| !v.is_deprecated())
            .filter_map(|(k, _)| Version::parse(k).ok())
            .filter(|v| v.pre.is_empty())
            .max()
    }
}

/// Errors from a [`DeprecationSource`].
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("registry request for {name} failed: {detail}")]
    Request { name: String, detail: String },
}

/// Supplies packuments; the registry client implements this, tests stub it.
pub trait DeprecationSource: Sync {
    /// `Ok(None)` = package unknown to the registry (treated not deprecated).
    ///
    /// # Errors
    /// [`SourceError`] on network/auth failure.
    fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_abbreviated_packument() {
        let p: Packument = serde_json::from_str(
            r#"{
            "dist-tags": {"latest": "2.1.0"},
            "versions": {
                "1.0.0": {"deprecated": "use @foo/core instead", "dependencies": {"x": "^1.0.0"}},
                "2.1.0": {}
            }
        }"#,
        )
        .unwrap();
        assert!(p.versions["1.0.0"].is_deprecated());
        assert_eq!(
            p.versions["1.0.0"].deprecation_message(),
            Some("use @foo/core instead")
        );
        assert!(!p.versions["2.1.0"].is_deprecated());
        assert_eq!(p.latest(), Some("2.1.0"));
    }

    #[test]
    fn boolean_deprecated_field_handled() {
        let p: Packument = serde_json::from_str(
            r#"{"versions": {"1.0.0": {"deprecated": true}, "1.1.0": {"deprecated": false}, "1.2.0": {"deprecated": ""}}}"#,
        )
        .unwrap();
        assert!(p.versions["1.0.0"].is_deprecated());
        assert!(!p.versions["1.1.0"].is_deprecated());
        assert!(!p.versions["1.2.0"].is_deprecated()); // empty string = un-deprecated
    }

    #[test]
    fn newest_non_deprecated_skips_deprecated_and_prerelease() {
        let p: Packument = serde_json::from_str(
            r#"{"versions": {
            "1.0.0": {}, "2.0.0": {"deprecated": "bad"}, "1.5.0": {}, "3.0.0-beta.1": {}
        }}"#,
        )
        .unwrap();
        assert_eq!(p.newest_non_deprecated().unwrap().to_string(), "1.5.0");
    }
}
