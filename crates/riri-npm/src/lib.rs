//! npm `package-lock.json` parser (v1, v2, v3).
//!
//! Parses the lockfile and exposes engine constraints per dependency
//! via the [`LockfileEngines`] trait.

use riri_common::{Engines, LockfileEngines};
use serde::Deserialize;
use std::collections::HashMap;

/// Errors that can occur when parsing an npm lockfile.
#[derive(Debug, thiserror::Error)]
pub enum NpmParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing lockfileVersion field")]
    MissingVersion,
    #[error("unsupported npm lockfile version: {0}")]
    UnsupportedVersion(u64),
}

/// A single dependency entry in an npm lockfile.
#[derive(Debug, Clone, Deserialize)]
pub struct NpmLockEntry {
    #[serde(default)]
    pub engines: Option<Engines>,
}

/// Parsed npm `package-lock.json` covering v1, v2, and v3 formats.
#[derive(Debug, Clone)]
pub enum NpmPackageLock {
    V1 {
        dependencies: HashMap<String, NpmLockEntry>,
    },
    V2 {
        dependencies: HashMap<String, NpmLockEntry>,
        packages: Option<HashMap<String, NpmLockEntry>>,
    },
    V3 {
        packages: HashMap<String, NpmLockEntry>,
    },
}

impl NpmPackageLock {
    /// Parse a `package-lock.json` from its JSON string content.
    ///
    /// Detects the lockfile version and dispatches to the appropriate format.
    ///
    /// # Errors
    ///
    /// Returns [`NpmParseError`] if the JSON is invalid, the `lockfileVersion`
    /// field is missing, or the version is unsupported.
    pub fn parse(content: &str) -> Result<Self, NpmParseError> {
        let raw: serde_json::Value = serde_json::from_str(content)?;

        let version = raw
            .get("lockfileVersion")
            .and_then(serde_json::Value::as_u64)
            .ok_or(NpmParseError::MissingVersion)?;

        match version {
            1 => {
                let lock: NpmLockV1Raw = serde_json::from_value(raw)?;
                Ok(Self::V1 {
                    dependencies: lock.dependencies,
                })
            }
            2 => {
                let lock: NpmLockV2Raw = serde_json::from_value(raw)?;
                Ok(Self::V2 {
                    dependencies: lock.dependencies,
                    packages: lock.packages,
                })
            }
            3 => {
                let lock: NpmLockV3Raw = serde_json::from_value(raw)?;
                Ok(Self::V3 {
                    packages: lock.packages,
                })
            }
            v => Err(NpmParseError::UnsupportedVersion(v)),
        }
    }

    /// Returns a reference to the package entries to use for engine extraction.
    ///
    /// - v1: uses `dependencies`
    /// - v2: prefers `packages` if present, falls back to `dependencies`
    /// - v3: uses `packages`
    #[must_use]
    pub fn entries(&self) -> &HashMap<String, NpmLockEntry> {
        match self {
            Self::V2 {
                packages: Some(p), ..
            } => p,
            Self::V1 { dependencies } | Self::V2 { dependencies, .. } => dependencies,
            Self::V3 { packages } => packages,
        }
    }
}

impl LockfileEngines for NpmPackageLock {
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_> {
        Box::new(
            self.entries()
                .iter()
                .filter_map(|(name, entry)| entry.engines.as_ref().map(|e| (name.as_str(), e))),
        )
    }
}

// --- Internal raw serde types ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmLockV1Raw {
    #[allow(dead_code)]
    lockfile_version: u64,
    #[serde(default)]
    dependencies: HashMap<String, NpmLockEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmLockV2Raw {
    #[allow(dead_code)]
    lockfile_version: u64,
    #[serde(default)]
    dependencies: HashMap<String, NpmLockEntry>,
    #[serde(default)]
    packages: Option<HashMap<String, NpmLockEntry>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmLockV3Raw {
    #[allow(dead_code)]
    lockfile_version: u64,
    #[serde(default)]
    packages: HashMap<String, NpmLockEntry>,
}
