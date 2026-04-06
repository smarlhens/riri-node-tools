//! pnpm `pnpm-lock.yaml` parser (v5, v6, v7, v8, v9, v10, v11).
//!
//! Parses the lockfile and exposes engine constraints per dependency
//! via the [`LockfileEngines`] trait.

use riri_common::{Engines, LockfileEngines};
use serde::Deserialize;
use std::collections::HashMap;

/// Errors that can occur when parsing a pnpm lockfile.
#[derive(Debug, thiserror::Error)]
pub enum PnpmParseError {
    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_yml::Error),
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
    },
    /// v6 format (lockfileVersion 6.x — pnpm v8).
    /// Keys: `/name@version(peers)` or `/@scope/name@version(peers)`.
    V6 {
        packages: HashMap<String, PnpmPackageEntry>,
    },
    /// v9 format (lockfileVersion 9.x — pnpm v9/v10/v11).
    /// Engines in `packages` (keyed `name@version`, no leading `/`).
    /// `snapshots` holds per-peer-context data (not needed for engines).
    V9 {
        packages: HashMap<String, PnpmPackageEntry>,
    },
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
        let raw: serde_yml::Value = serde_yml::from_str(content)?;

        let version_str = raw
            .get("lockfileVersion")
            .map(|v| match v {
                serde_yml::Value::Number(n) => n.to_string(),
                serde_yml::Value::String(s) => s.clone(),
                other => format!("{other:?}"),
            })
            .ok_or(PnpmParseError::MissingVersion)?;

        let major = parse_major_version(&version_str)
            .ok_or_else(|| PnpmParseError::UnsupportedVersion(version_str.clone()))?;

        match major {
            5 => {
                let lock: PnpmLockV5Raw = serde_yml::from_value(raw)?;
                Ok(Self::V5 {
                    packages: lock.packages,
                })
            }
            6 => {
                let lock: PnpmLockV6Raw = serde_yml::from_value(raw)?;
                Ok(Self::V6 {
                    packages: lock.packages,
                })
            }
            9 => {
                let lock: PnpmLockV9Raw = serde_yml::from_value(raw)?;
                Ok(Self::V9 {
                    packages: lock.packages,
                })
            }
            _ => Err(PnpmParseError::UnsupportedVersion(version_str)),
        }
    }

    /// Returns a reference to the package entries for engine extraction.
    #[must_use]
    pub fn entries(&self) -> &HashMap<String, PnpmPackageEntry> {
        match self {
            Self::V5 { packages } | Self::V6 { packages } | Self::V9 { packages } => packages,
        }
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

/// Extract the integer major version from a version string like `"5.4"` or `"9.0"`.
fn parse_major_version(version: &str) -> Option<u64> {
    let clean = version.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
    clean.split('.').next()?.parse().ok()
}

// --- Internal raw serde types ---

#[derive(Deserialize)]
struct PnpmLockV5Raw {
    #[allow(dead_code)]
    #[serde(alias = "lockfileVersion")]
    lockfile_version: serde_yml::Value,
    #[serde(default)]
    packages: HashMap<String, PnpmPackageEntry>,
}

#[derive(Deserialize)]
struct PnpmLockV6Raw {
    #[allow(dead_code)]
    #[serde(alias = "lockfileVersion")]
    lockfile_version: serde_yml::Value,
    #[serde(default)]
    packages: HashMap<String, PnpmPackageEntry>,
}

#[derive(Deserialize)]
struct PnpmLockV9Raw {
    #[allow(dead_code)]
    #[serde(alias = "lockfileVersion")]
    lockfile_version: serde_yml::Value,
    #[serde(default)]
    packages: HashMap<String, PnpmPackageEntry>,
    #[allow(dead_code)]
    #[serde(default)]
    snapshots: Option<serde_yml::Value>,
}
