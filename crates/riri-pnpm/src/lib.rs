//! pnpm `pnpm-lock.yaml` parser (v5, v6, v7, v8, v9, v10, v11).
//!
//! Parses the lockfile and exposes engine constraints per dependency
//! via the [`LockfileEngines`] trait.

pub mod catalog;

use riri_common::{Engines, LockfileEngines, LockfileVersions};
use serde::Deserialize;
use std::collections::HashMap;

/// Errors that can occur when parsing a pnpm lockfile.
#[derive(Debug, thiserror::Error)]
pub enum PnpmParseError {
    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_saphyr::Error),
    #[error("missing lockfileVersion field")]
    MissingVersion,
    #[error("unsupported pnpm lockfile version: {0}")]
    UnsupportedVersion(String),
}

/// A single package entry in a pnpm lockfile.
#[derive(Debug, Clone, Deserialize)]
pub struct PnpmPackageEntry {
    #[serde(default)]
    pub engines: Option<Engines>,
}

/// Parsed pnpm `pnpm-lock.yaml` covering all supported format versions.
#[derive(Debug, Clone)]
pub enum PnpmLockfile {
    /// v5 format (lockfileVersion 5.x — pnpm v5/v6/v7).
    /// Keys: `/name/version` or `/@scope/name/version`.
    V5 {
        packages: HashMap<String, PnpmPackageEntry>,
        importers: HashMap<String, PnpmImporter>,
    },
    /// v6 format (lockfileVersion 6.x — pnpm v8).
    /// Keys: `/name@version(peers)` or `/@scope/name@version(peers)`.
    V6 {
        packages: HashMap<String, PnpmPackageEntry>,
        importers: HashMap<String, PnpmImporter>,
    },
    /// v9 format (lockfileVersion 9.x — pnpm v9/v10/v11).
    /// Engines in `packages` (keyed `name@version`, no leading `/`).
    /// `snapshots` holds per-peer-context data (not needed for engines).
    V9 {
        packages: HashMap<String, PnpmPackageEntry>,
        importers: HashMap<String, PnpmImporter>,
    },
}

/// One importer (workspace member) entry — most projects only have `.`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PnpmImporter {
    #[serde(default)]
    pub dependencies: HashMap<String, ImporterDep>,
    #[serde(default, rename = "devDependencies")]
    pub dev_dependencies: HashMap<String, ImporterDep>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, ImporterDep>,
}

/// Either a v5 plain version string (`"1.2.3"`) or a v6+ object with
/// `specifier` + `version` fields. The `version` is what the lockfile
/// resolved to (used for pinning).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ImporterDep {
    /// v5: value is just the resolved version string.
    Version(String),
    /// v6+: object form. `version` may carry peer suffix like `1.2.3(peer@18.0.0)`.
    Object {
        #[serde(default)]
        specifier: Option<String>,
        version: String,
    },
}

impl ImporterDep {
    /// Returns the resolved version, stripped of any pnpm peer-context suffix.
    #[must_use]
    pub fn version(&self) -> &str {
        let raw = match self {
            Self::Version(v) | Self::Object { version: v, .. } => v.as_str(),
        };
        raw.split_once('(').map_or(raw, |(v, _)| v)
    }
}

impl PnpmLockfile {
    /// Parse a `pnpm-lock.yaml` from its YAML string content.
    ///
    /// Detects the lockfile version and dispatches to the appropriate format.
    ///
    /// # Errors
    ///
    /// Returns [`PnpmParseError`] if the YAML is invalid, the `lockfileVersion`
    /// field is missing, or the version is unsupported.
    pub fn parse(content: &str) -> Result<Self, PnpmParseError> {
        // pnpm v11 uses multi-document YAML (env lockfile + main lockfile).
        // The main lockfile is always the last document.
        let yaml_content = content
            .rsplit_once("\n---")
            .map_or(content, |(_, last)| last);

        // First pass: detect lockfile version.
        let detect: VersionDetect = serde_saphyr::from_str(yaml_content).map_err(|e| {
            // Distinguish genuine YAML syntax errors from a missing field:
            // re-parse into a throwaway type — if *that* also fails the YAML is broken.
            if serde_saphyr::from_str::<serde::de::IgnoredAny>(yaml_content).is_err() {
                PnpmParseError::Yaml(e)
            } else {
                PnpmParseError::MissingVersion
            }
        })?;

        let version_str = match detect.lockfile_version {
            VersionValue::Number(n) => n.to_string(),
            VersionValue::String(s) => s,
        };

        let major = parse_major_version(&version_str)
            .ok_or_else(|| PnpmParseError::UnsupportedVersion(version_str.clone()))?;

        // Second pass: deserialize the full lockfile into the correct typed struct.
        match major {
            5 => {
                let lock: PnpmLockPackages = serde_saphyr::from_str(yaml_content)?;
                Ok(Self::V5 {
                    packages: lock.packages,
                    importers: lock.importers,
                })
            }
            6 => {
                let lock: PnpmLockPackages = serde_saphyr::from_str(yaml_content)?;
                Ok(Self::V6 {
                    packages: lock.packages,
                    importers: lock.importers,
                })
            }
            9 => {
                let lock: PnpmLockPackages = serde_saphyr::from_str(yaml_content)?;
                Ok(Self::V9 {
                    packages: lock.packages,
                    importers: lock.importers,
                })
            }
            _ => Err(PnpmParseError::UnsupportedVersion(version_str)),
        }
    }

    /// Returns a reference to the package entries for engine extraction.
    #[must_use]
    pub fn entries(&self) -> &HashMap<String, PnpmPackageEntry> {
        match self {
            Self::V5 { packages, .. } | Self::V6 { packages, .. } | Self::V9 { packages, .. } => {
                packages
            }
        }
    }

    /// Returns the root importer (`.`), which lists the directly-declared
    /// dependencies of `package.json` and their resolved versions.
    #[must_use]
    pub fn root_importer(&self) -> Option<&PnpmImporter> {
        let importers = match self {
            Self::V5 { importers, .. }
            | Self::V6 { importers, .. }
            | Self::V9 { importers, .. } => importers,
        };
        importers.get(".")
    }
}

impl LockfileEngines for PnpmLockfile {
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_> {
        Box::new(
            self.entries()
                .iter()
                .filter_map(|(name, entry)| entry.engines.as_ref().map(|e| (name.as_str(), e))),
        )
    }
}

impl LockfileVersions for PnpmLockfile {
    fn version_for(&self, name: &str) -> Option<&str> {
        if let Some(importer) = self.root_importer()
            && let Some(dep) = importer
                .dependencies
                .get(name)
                .or_else(|| importer.dev_dependencies.get(name))
                .or_else(|| importer.optional_dependencies.get(name))
        {
            return Some(dep.version());
        }
        let importers = match self {
            Self::V5 { importers, .. }
            | Self::V6 { importers, .. }
            | Self::V9 { importers, .. } => importers,
        };
        importers
            .iter()
            .filter(|(k, _)| k.as_str() != ".")
            .find_map(|(_, importer)| {
                importer
                    .dependencies
                    .get(name)
                    .or_else(|| importer.dev_dependencies.get(name))
                    .or_else(|| importer.optional_dependencies.get(name))
                    .map(ImporterDep::version)
            })
    }
}

#[cfg(test)]
mod versions_tests {
    use super::*;

    #[test]
    fn version_strips_pnpm_peer_suffix() {
        let dep = ImporterDep::Object {
            specifier: Some("^1.0.0".into()),
            version: "1.6.0(qux@20.0.0)".into(),
        };
        assert_eq!(dep.version(), "1.6.0");
    }

    #[test]
    fn version_returns_plain_v5_string_unchanged() {
        let dep = ImporterDep::Version("4.17.21".into());
        assert_eq!(dep.version(), "4.17.21");
    }

    #[test]
    fn version_for_resolves_dev_and_optional() {
        let v9 = "
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      foo:
        specifier: ^4.0.0
        version: 4.17.21
    devDependencies:
      baz:
        specifier: ^1.0.0
        version: 1.6.0
    optionalDependencies:
      qux:
        specifier: ^2.3.0
        version: 2.3.3
packages: {}
";
        let lock = PnpmLockfile::parse(v9).expect("parse");
        assert_eq!(lock.version_for("foo"), Some("4.17.21"));
        assert_eq!(lock.version_for("baz"), Some("1.6.0"));
        assert_eq!(lock.version_for("qux"), Some("2.3.3"));
        assert_eq!(lock.version_for("missing"), None);
    }

    #[test]
    fn version_for_handles_v5_string_form() {
        let v5 = "
lockfileVersion: 5.4
importers:
  .:
    dependencies:
      foo: 4.17.21
packages: {}
";
        let lock = PnpmLockfile::parse(v5).expect("parse");
        assert_eq!(lock.version_for("foo"), Some("4.17.21"));
    }
}

/// Extract the integer major version from a version string like `"5.4"` or `"9.0"`.
fn parse_major_version(version: &str) -> Option<u64> {
    let clean = version.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
    clean.split('.').next()?.parse().ok()
}

// --- Internal serde types ---

/// Handles `lockfileVersion` being either a YAML number (5.4) or a quoted string ('6.0').
#[derive(Deserialize)]
#[serde(untagged)]
enum VersionValue {
    Number(f64),
    String(String),
}

/// Minimal struct for version detection (first pass).
#[derive(Deserialize)]
struct VersionDetect {
    #[serde(alias = "lockfileVersion")]
    lockfile_version: VersionValue,
}

/// Shared struct for all lockfile versions.
/// `#[serde(deny_unknown_fields)]` is NOT used so extra fields
/// (like `snapshots`, `settings`) are silently ignored.
#[derive(Deserialize)]
struct PnpmLockPackages {
    #[serde(default)]
    packages: HashMap<String, PnpmPackageEntry>,
    #[serde(default)]
    importers: HashMap<String, PnpmImporter>,
}
