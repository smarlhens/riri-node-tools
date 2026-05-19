//! Core logic for `npm-pin-dependencies`.
//!
//! Compares each entry of `package.json` `dependencies`/`devDependencies`/
//! `optionalDependencies` against the lockfile and reports any range
//! specifier (e.g. `^1.2.3`) that should be pinned to the resolved version.

pub mod cli;

use riri_common::{LockfileVersions, PackageJson};
use semver::Version;
use thiserror::Error;
use tracing::debug;

/// A dependency whose `package.json` specifier is a range that the lockfile
/// has resolved to a concrete version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionToPin {
    /// The dependency name as it appears in `package.json`.
    pub name: String,
    /// The dependency category (`dependencies`, `devDependencies`, …).
    pub kind: DependencyKind,
    /// The unpinned specifier currently in `package.json`.
    pub current_range: String,
    /// The exact version the lockfile resolved for this dependency.
    pub pinned_version: String,
}

/// The `package.json` dependency category a [`VersionToPin`] originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Dependencies,
    DevDependencies,
    OptionalDependencies,
}

impl DependencyKind {
    /// Returns the JSON object key corresponding to this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "devDependencies",
            Self::OptionalDependencies => "optionalDependencies",
        }
    }
}

/// Errors raised by [`pin_dependencies`].
///
/// Currently a placeholder enum — pinning itself never fails, but a typed
/// error surface keeps room for future failure modes (e.g. yarn `node_modules`
/// not installed).
#[derive(Debug, Error)]
pub enum PinError {}

/// Compares `package.json` against `lockfile` and returns every dependency
/// whose specifier is a range that the lockfile has resolved to a concrete
/// version.
///
/// Skip rules:
///   - Dependencies prefixed with `file:`, `link:`, or `workspace:` (local).
///   - Dependencies whose specifier already parses as a [`Version`] AND
///     equals the lockfile's resolved version.
///   - Dependencies absent from the lockfile (treated as unresolved, ignored).
///
/// # Errors
///
/// Returns [`PinError`] for future failure modes; current implementation is
/// always `Ok`.
pub fn pin_dependencies(
    package_json: &PackageJson,
    lockfile: &dyn LockfileVersions,
) -> Result<Vec<VersionToPin>, PinError> {
    let mut result = Vec::new();
    let buckets: [(DependencyKind, _); 3] = [
        (DependencyKind::Dependencies, &package_json.dependencies),
        (
            DependencyKind::DevDependencies,
            &package_json.dev_dependencies,
        ),
        (
            DependencyKind::OptionalDependencies,
            &package_json.optional_dependencies,
        ),
    ];

    for (kind, deps) in buckets {
        let Some(deps) = deps else { continue };
        let mut entries: Vec<(&String, &String)> = deps.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (name, current_range) in entries {
            if is_local_specifier(current_range) {
                debug!(
                    target: "riri_npd::pin",
                    bucket = kind.as_str(),
                    package = %name,
                    spec = %current_range,
                    "Skipped local specifier (file:/link:/workspace:)"
                );
                continue;
            }
            let Some(locked) = lockfile.version_for(name) else {
                debug!(
                    target: "riri_npd::pin",
                    bucket = kind.as_str(),
                    package = %name,
                    spec = %current_range,
                    "Skipped — not present in lockfile"
                );
                continue;
            };
            if is_already_pinned(current_range, locked) {
                debug!(
                    target: "riri_npd::pin",
                    bucket = kind.as_str(),
                    package = %name,
                    spec = %current_range,
                    locked = %locked,
                    "Skipped — already pinned to lockfile version"
                );
                continue;
            }
            debug!(
                target: "riri_npd::pin",
                bucket = kind.as_str(),
                package = %name,
                "Pin {current_range} → {locked}"
            );
            result.push(VersionToPin {
                name: name.clone(),
                kind,
                current_range: current_range.clone(),
                pinned_version: locked.to_string(),
            });
        }
    }

    result.sort_by(|a, b| {
        a.kind
            .as_str()
            .cmp(b.kind.as_str())
            .then(a.name.cmp(&b.name))
    });
    Ok(result)
}

fn is_local_specifier(spec: &str) -> bool {
    spec.starts_with("file:") || spec.starts_with("link:") || spec.starts_with("workspace:")
}

fn is_already_pinned(spec: &str, locked: &str) -> bool {
    Version::parse(spec).is_ok_and(|parsed| parsed.to_string() == locked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct StubLockfile {
        versions: HashMap<String, String>,
    }

    impl LockfileVersions for StubLockfile {
        fn version_for(&self, name: &str) -> Option<&str> {
            self.versions.get(name).map(String::as_str)
        }
    }

    fn pkg(deps: &[(&str, &str)]) -> PackageJson {
        PackageJson {
            name: None,
            version: None,
            engines: None,
            dependencies: Some(
                deps.iter()
                    .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
                    .collect(),
            ),
            dev_dependencies: None,
            optional_dependencies: None,
        }
    }

    fn locked(entries: &[(&str, &str)]) -> StubLockfile {
        StubLockfile {
            versions: entries
                .iter()
                .map(|(n, v)| ((*n).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    #[test]
    fn pins_unpinned_caret_range_to_lockfile_version() {
        let pkg = pkg(&[("foo", "^1.2.3")]);
        let lock = locked(&[("foo", "1.2.5")]);
        let result = pin_dependencies(&pkg, &lock).expect("ok");
        assert_eq!(
            result,
            vec![VersionToPin {
                name: "foo".into(),
                kind: DependencyKind::Dependencies,
                current_range: "^1.2.3".into(),
                pinned_version: "1.2.5".into(),
            }]
        );
    }

    #[test]
    fn skips_dependency_already_pinned_to_lockfile_version() {
        let pkg = pkg(&[("foo", "1.2.5")]);
        let lock = locked(&[("foo", "1.2.5")]);
        assert!(pin_dependencies(&pkg, &lock).expect("ok").is_empty());
    }

    #[test]
    fn pins_when_specifier_parses_but_differs_from_lockfile() {
        let pkg = pkg(&[("foo", "1.2.3")]);
        let lock = locked(&[("foo", "1.2.5")]);
        let result = pin_dependencies(&pkg, &lock).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pinned_version, "1.2.5");
    }

    #[test]
    fn skips_file_link_workspace_specifiers() {
        let pkg = pkg(&[
            ("local", "file:../local-pkg"),
            ("linked", "link:../shared"),
            ("ws", "workspace:*"),
        ]);
        let lock = locked(&[("local", "1.0.0"), ("linked", "1.0.0"), ("ws", "1.0.0")]);
        assert!(pin_dependencies(&pkg, &lock).expect("ok").is_empty());
    }

    #[test]
    fn skips_dependency_absent_from_lockfile() {
        let pkg = pkg(&[("missing", "^1.0.0")]);
        let lock = locked(&[]);
        assert!(pin_dependencies(&pkg, &lock).expect("ok").is_empty());
    }

    #[test]
    fn separates_dependencies_dev_and_optional() {
        let mut pkg = pkg(&[("a", "^1.0.0")]);
        pkg.dev_dependencies = Some(
            [("b".to_string(), "^2.0.0".to_string())]
                .into_iter()
                .collect(),
        );
        pkg.optional_dependencies = Some(
            [("c".to_string(), "^3.0.0".to_string())]
                .into_iter()
                .collect(),
        );
        let lock = locked(&[("a", "1.0.1"), ("b", "2.0.1"), ("c", "3.0.1")]);
        let result = pin_dependencies(&pkg, &lock).expect("ok");
        assert_eq!(result.len(), 3);
        let kinds: Vec<_> = result.iter().map(|v| v.kind).collect();
        assert!(kinds.contains(&DependencyKind::Dependencies));
        assert!(kinds.contains(&DependencyKind::DevDependencies));
        assert!(kinds.contains(&DependencyKind::OptionalDependencies));
    }
}
