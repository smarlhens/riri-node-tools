mod detect;

pub use detect::{DetectError, detect_lockfile, find_package_json};

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Package manager kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

/// Result of discovering a lockfile on disk.
#[derive(Debug, Clone)]
pub struct LockFileResult {
    pub path: PathBuf,
    pub package_manager: PackageManager,
}

/// Engine constraint keys supported by Node.js tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineConstraintKey {
    Node,
    Npm,
    Yarn,
}

impl fmt::Display for EngineConstraintKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node => write!(f, "node"),
            Self::Npm => write!(f, "npm"),
            Self::Yarn => write!(f, "yarn"),
        }
    }
}

impl EngineConstraintKey {
    /// Parse from a lowercase string (e.g. `"node"`, `"npm"`, `"yarn"`).
    #[must_use]
    pub fn from_str_lowercase(s: &str) -> Option<Self> {
        match s {
            "node" => Some(Self::Node),
            "npm" => Some(Self::Npm),
            "yarn" => Some(Self::Yarn),
            _ => None,
        }
    }
}

/// Engines field as found in lockfiles — can be an object or an array.
///
/// Object: `{ "node": ">=16.0.0", "npm": ">=8.0.0" }`
/// Array: `["node >= 16"]`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Engines {
    Object(HashMap<String, String>),
    Array(Vec<String>),
}

/// Shared `package.json` representation covering fields needed by both
/// `npm-check-engines` (engines) and `npm-pin-dependencies` (dependencies).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub engines: Option<HashMap<String, String>>,
    #[serde(default)]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub optional_dependencies: Option<HashMap<String, String>>,
}

/// Unified trait for extracting engine constraints from any lockfile format.
///
/// Each package manager crate implements this for its own lockfile type.
pub trait LockfileEngines {
    /// Iterate over `(package_name, engines)` pairs from the lockfile.
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_>;
}
