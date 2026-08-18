//! Parse any supported lockfile from a string, so consumers stop matching on
//! package-manager kind and depending on all three parser crates to do it.

#[cfg(feature = "graph")]
use riri_common::LockfileGraph;
use riri_common::{LockfileEngines, LockfileVersions, PackageManager};
use riri_npm::{NpmPackageLock, NpmParseError};
use riri_pnpm::{PnpmLockfile, PnpmParseError};
use riri_yarn::{YarnLock, YarnParseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockfileParseError {
    #[error(transparent)]
    Npm(#[from] NpmParseError),
    #[error(transparent)]
    Pnpm(#[from] PnpmParseError),
    #[error(transparent)]
    Yarn(#[from] YarnParseError),
}

/// A parsed lockfile of any supported format.
#[derive(Debug)]
pub enum ParsedLockfile {
    Npm(NpmPackageLock),
    Pnpm(PnpmLockfile),
    Yarn(YarnLock),
}

/// Parse `content` as the lockfile format `manager` writes.
///
/// # Errors
///
/// Returns [`LockfileParseError`] when the content is not a valid lockfile of
/// that format, wrapping the format-specific parse error.
pub fn parse_lockfile(
    manager: PackageManager,
    content: &str,
) -> Result<ParsedLockfile, LockfileParseError> {
    Ok(match manager {
        PackageManager::Npm => ParsedLockfile::Npm(NpmPackageLock::parse(content)?),
        PackageManager::Pnpm => ParsedLockfile::Pnpm(PnpmLockfile::parse(content)?),
        PackageManager::Yarn => ParsedLockfile::Yarn(YarnLock::parse(content)?),
    })
}

impl ParsedLockfile {
    #[must_use]
    pub fn versions(&self) -> &dyn LockfileVersions {
        match self {
            Self::Npm(lock) => lock,
            Self::Pnpm(lock) => lock,
            Self::Yarn(lock) => lock,
        }
    }

    /// `None` for yarn: `yarn.lock` stores no `engines` at any version, so the
    /// install tree is the only source — `riri_yarn::YarnProject` (feature
    /// `scan`), which reads `node_modules` and so cannot live behind a pure
    /// entry point.
    /// The dependency graph, which every format can produce.
    #[cfg(feature = "graph")]
    #[must_use]
    pub fn graph(&self) -> &dyn LockfileGraph {
        match self {
            Self::Npm(lock) => lock,
            Self::Pnpm(lock) => lock,
            Self::Yarn(lock) => lock,
        }
    }

    #[must_use]
    pub fn engines(&self) -> Option<&dyn LockfileEngines> {
        match self {
            Self::Npm(lock) => Some(lock),
            Self::Pnpm(lock) => Some(lock),
            Self::Yarn(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_lockfile;
    use riri_common::PackageManager;

    const NPM_V3: &str = r#"{
        "lockfileVersion": 3,
        "packages": {
            "": {"name": "app"},
            "node_modules/typescript": {"version": "5.4.5", "engines": {"node": ">=14.17"}}
        }
    }"#;

    const PNPM_V9: &str = "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n      typescript:\n        specifier: ^5.4.0\n        version: 5.4.5\npackages:\n  typescript@5.4.5:\n    engines: {node: '>=14.17'}\n";

    const YARN_V1: &str = "# yarn lockfile v1\n\n\ntypescript@^5.4.0:\n  version \"5.4.5\"\n";

    #[test]
    fn parses_npm_versions_and_engines() {
        let lock = parse_lockfile(PackageManager::Npm, NPM_V3).expect("parses");
        assert_eq!(lock.versions().version_for("typescript"), Some("5.4.5"));
        let engines = lock.engines().expect("npm lockfiles carry engines");
        assert_eq!(
            engines.engines_iter().count(),
            1,
            "one entry declares engines"
        );
    }

    #[test]
    fn parses_pnpm_versions_and_engines() {
        let lock = parse_lockfile(PackageManager::Pnpm, PNPM_V9).expect("parses");
        assert_eq!(lock.versions().version_for("typescript"), Some("5.4.5"));
        assert!(lock.engines().is_some());
    }

    #[test]
    fn parses_yarn_versions_but_reports_no_engines() {
        let lock = parse_lockfile(PackageManager::Yarn, YARN_V1).expect("parses");
        assert_eq!(
            lock.versions().resolved_version("typescript", "^5.4.0"),
            Some("5.4.5")
        );
        assert!(
            lock.engines().is_none(),
            "yarn.lock stores no engines at any version"
        );
    }

    #[test]
    fn wrong_format_for_the_content_is_an_error() {
        assert!(parse_lockfile(PackageManager::Npm, YARN_V1).is_err());
    }
}
