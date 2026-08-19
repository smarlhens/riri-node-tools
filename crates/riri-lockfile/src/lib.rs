//! Parse any supported lockfile from a string, so consumers stop matching on
//! package-manager kind and depending on all three parser crates to do it.

#[cfg(feature = "graph")]
use riri_common::LockfileGraph;
use riri_common::{LockfileEngines, LockfileVersions, PackageManager};
use riri_npm::{NpmPackageLock, NpmParseError};
use riri_pnpm::{PnpmLockfile, PnpmParseError};
use riri_yarn::{YarnLock, YarnParseError};
#[cfg(feature = "scan")]
use riri_yarn::{YarnProject, YarnScanError};
#[cfg(feature = "scan")]
use std::path::Path;
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

    /// Engine constraints recorded in the lockfile itself. `None` for yarn,
    /// which records none — see [`open_engines`] (feature `scan`).
    #[must_use]
    pub fn engines(&self) -> Option<&dyn LockfileEngines> {
        match self {
            Self::Npm(lock) => Some(lock),
            Self::Pnpm(lock) => Some(lock),
            Self::Yarn(_) => None,
        }
    }
}

/// Errors from [`open_engines`].
#[cfg(feature = "scan")]
#[derive(Debug, Error)]
pub enum OpenEnginesError {
    #[error(transparent)]
    Parse(#[from] LockfileParseError),
    #[error(transparent)]
    Scan(#[from] YarnScanError),
}

/// An engine source, owning whatever it was read from.
#[cfg(feature = "scan")]
#[derive(Debug)]
pub enum OpenedEngines {
    Npm(NpmPackageLock),
    Pnpm(PnpmLockfile),
    /// Scanned from `node_modules`, which is where yarn keeps them.
    InstallTree(YarnProject),
}

#[cfg(feature = "scan")]
impl OpenedEngines {
    #[must_use]
    pub fn engines(&self) -> &dyn LockfileEngines {
        match self {
            Self::Npm(lock) => lock,
            Self::Pnpm(lock) => lock,
            Self::InstallTree(project) => project,
        }
    }
}

/// Engine constraints for any package manager.
///
/// npm and pnpm are parsed from `content`. yarn records none, so it is scanned
/// from `{lockfile_path parent}/node_modules` — the only I/O here, and the
/// reason for the `scan` feature.
///
/// # Errors
///
/// [`OpenEnginesError::Parse`] when `content` is not that format,
/// [`OpenEnginesError::Scan`] when yarn's `node_modules` is missing.
#[cfg(feature = "scan")]
pub fn open_engines(
    manager: PackageManager,
    lockfile_path: &Path,
    content: &str,
) -> Result<OpenedEngines, OpenEnginesError> {
    Ok(match manager {
        PackageManager::Npm => {
            OpenedEngines::Npm(NpmPackageLock::parse(content).map_err(LockfileParseError::Npm)?)
        }
        PackageManager::Pnpm => {
            OpenedEngines::Pnpm(PnpmLockfile::parse(content).map_err(LockfileParseError::Pnpm)?)
        }
        PackageManager::Yarn => {
            let project_dir = lockfile_path.parent().unwrap_or_else(|| Path::new("."));
            OpenedEngines::InstallTree(YarnProject::scan(project_dir)?)
        }
    })
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

    #[cfg(feature = "scan")]
    mod open_engines {
        use super::{NPM_V3, PNPM_V9, YARN_V1};
        use crate::{OpenEnginesError, open_engines};
        use riri_common::PackageManager;
        use std::path::Path;

        #[test]
        fn npm_and_pnpm_read_engines_out_of_the_content() {
            for (manager, content) in [
                (PackageManager::Npm, NPM_V3),
                (PackageManager::Pnpm, PNPM_V9),
            ] {
                // The path is unused for these two: nothing on disk to find.
                let opened = open_engines(manager, Path::new("/nonexistent/lock"), content)
                    .expect("parses from content alone");
                assert_eq!(
                    opened.engines().engines_iter().count(),
                    1,
                    "{manager:?} records engines in the lockfile"
                );
            }
        }

        #[test]
        fn yarn_reads_engines_out_of_node_modules() {
            let project = tempfile::TempDir::new().expect("tempdir");
            let pkg_dir = project.path().join("node_modules/typescript");
            std::fs::create_dir_all(&pkg_dir).expect("create node_modules");
            std::fs::write(
                pkg_dir.join("package.json"),
                r#"{"version": "5.4.5", "engines": {"node": ">=14.17"}}"#,
            )
            .expect("write package.json");

            let opened = open_engines(
                PackageManager::Yarn,
                &project.path().join("yarn.lock"),
                YARN_V1,
            )
            .expect("scans the install tree");

            let entries: Vec<_> = opened.engines().engines_iter().collect();
            assert_eq!(entries.len(), 1, "one installed package declares engines");
            assert_eq!(entries[0].0, "typescript");
        }

        #[test]
        fn yarn_without_node_modules_is_a_scan_error() {
            let project = tempfile::TempDir::new().expect("tempdir");
            let error = open_engines(
                PackageManager::Yarn,
                &project.path().join("yarn.lock"),
                YARN_V1,
            )
            .expect_err("no install tree to scan");
            assert!(matches!(error, OpenEnginesError::Scan(_)));
        }

        #[test]
        fn wrong_format_for_the_content_is_a_parse_error() {
            let error = open_engines(PackageManager::Npm, Path::new("lock"), YARN_V1)
                .expect_err("yarn.lock is not package-lock.json");
            assert!(matches!(error, OpenEnginesError::Parse(_)));
        }
    }
}
