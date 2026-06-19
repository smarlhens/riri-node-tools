mod detect;
mod npmrc;
mod package_json_file;

pub use detect::{DetectError, detect_lockfile, find_package_json};
#[cfg(feature = "graph")]
pub use npmrc::{DEFAULT_REGISTRY, NpmrcRegistryConfig};
pub use npmrc::{NpmrcOutcome, upsert_npmrc_flag};
pub use package_json_file::{PackageJsonFile, to_pretty_json_preserving_indent};

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

/// Workspaces field as found in `package.json`. npm + yarn accept both
/// `["apps/*"]` and `{"packages": ["apps/*"], "nohoist": [...]}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum WorkspacesField {
    Array(Vec<String>),
    Object {
        #[serde(default)]
        packages: Vec<String>,
        #[serde(default)]
        nohoist: Vec<String>,
    },
}

impl WorkspacesField {
    #[must_use]
    pub fn packages(&self) -> Vec<String> {
        match self {
            Self::Array(v) => v.clone(),
            Self::Object { packages, .. } => packages.clone(),
        }
    }
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
    #[serde(default)]
    pub workspaces: Option<WorkspacesField>,
}

/// Unified trait for extracting engine constraints from any lockfile format.
///
/// Each package manager crate implements this for its own lockfile type.
pub trait LockfileEngines {
    /// Iterate over `(package_name, engines)` pairs from the lockfile.
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_>;
}

/// Unified trait for resolving the locked version of a top-level dependency.
///
/// Used by `riri-npd` to look up "what version did the lockfile pin for `foo`?"
/// regardless of the underlying package manager.
pub trait LockfileVersions {
    /// Returns the locked version for the given top-level `package.json`
    /// dependency name, or `None` when the lockfile does not pin that name.
    fn version_for(&self, name: &str) -> Option<&str>;

    /// Resolves the locked version using the `package.json` `range` specifier
    /// in addition to the `name`.
    ///
    /// npm and pnpm lockfiles are name-keyed, so the default ignores `range`
    /// and delegates to [`Self::version_for`]. yarn lockfiles are keyed by the
    /// `name@range` descriptor and override this to resolve precisely.
    fn resolved_version(&self, name: &str, _range: &str) -> Option<&str> {
        self.version_for(name)
    }
}

/// Which section of `package.json` a root dependency comes from.
#[cfg(feature = "graph")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootDepKind {
    Dependencies,
    DevDependencies,
    OptionalDependencies,
}

#[cfg(feature = "graph")]
impl RootDepKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "devDependencies",
            Self::OptionalDependencies => "optionalDependencies",
        }
    }
}

/// A node in the resolved dependency graph — one *installed instance*.
#[cfg(feature = "graph")]
#[derive(Debug, Clone)]
pub struct LockGraphNode {
    pub name: String,
    pub version: String,
    pub deps: Vec<LockGraphEdge>,
}

/// A directed edge from a parent node to a child node in the graph.
#[cfg(feature = "graph")]
#[derive(Debug, Clone)]
pub struct LockGraphEdge {
    /// Index into [`LockGraph::nodes`].
    pub node: usize,
    pub optional: bool,
}

/// A root dependency edge (from the project root to a top-level package).
#[cfg(feature = "graph")]
#[derive(Debug, Clone)]
pub struct LockGraphRoot {
    pub kind: RootDepKind,
    /// The `package.json` specifier range (e.g. `^1.2.3`).
    pub range: String,
    /// Index into [`LockGraph::nodes`].
    pub node: usize,
}

/// Resolved dependency graph extracted from a lockfile.
#[cfg(feature = "graph")]
#[derive(Debug, Clone, Default)]
pub struct LockGraph {
    pub nodes: Vec<LockGraphNode>,
    pub roots: Vec<LockGraphRoot>,
}

/// Error type for lockfile graph extraction.
#[cfg(feature = "graph")]
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("{0}")]
    Other(String),
}

/// Trait implemented by each lockfile parser to expose the full dependency graph.
#[cfg(feature = "graph")]
pub trait LockfileGraph {
    /// Build a dependency graph from a parsed lockfile. `package_json` supplies
    /// the direct-dependency specifiers (and, for yarn, the root descriptors).
    /// Unresolvable edges are skipped, not errors — lockfiles legitimately omit
    /// optional/platform-specific entries.
    ///
    /// # Errors
    /// Returns [`GraphError`] for structurally broken lockfiles.
    fn dep_graph(&self, package_json: &PackageJson) -> Result<LockGraph, GraphError>;
}

/// Returns `true` if the specifier refers to a local path or workspace alias
/// rather than a registry package.
#[must_use]
pub fn is_local_specifier(spec: &str) -> bool {
    spec.starts_with("file:")
        || spec.starts_with("link:")
        || spec.starts_with("workspace:")
        || spec.starts_with("catalog:")
        || spec == "catalog"
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use super::*;

    #[cfg(feature = "graph")]
    #[test]
    fn root_dep_kind_as_str() {
        assert_eq!(RootDepKind::Dependencies.as_str(), "dependencies");
        assert_eq!(RootDepKind::DevDependencies.as_str(), "devDependencies");
        assert_eq!(
            RootDepKind::OptionalDependencies.as_str(),
            "optionalDependencies"
        );
    }

    #[test]
    fn is_local_specifier_recognizes_all_forms() {
        assert!(is_local_specifier("file:../foo"));
        assert!(is_local_specifier("link:../bar"));
        assert!(is_local_specifier("workspace:*"));
        assert!(is_local_specifier("catalog:react18"));
        assert!(is_local_specifier("catalog"));
        assert!(!is_local_specifier("^1.2.3"));
        assert!(!is_local_specifier("1.2.3"));
        assert!(!is_local_specifier("latest"));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod workspaces_tests {
    use super::*;

    #[test]
    fn parses_array_form() {
        let json = r#"{"workspaces": ["apps/*", "packages/*"]}"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(
            pkg.workspaces.unwrap().packages(),
            vec!["apps/*".to_string(), "packages/*".to_string()]
        );
    }

    #[test]
    fn parses_object_form() {
        let json = r#"{"workspaces": {"packages": ["apps/*"], "nohoist": ["foo"]}}"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(
            pkg.workspaces.unwrap().packages(),
            vec!["apps/*".to_string()]
        );
    }

    #[test]
    fn missing_workspaces_is_none() {
        let pkg: PackageJson = serde_json::from_str(r#"{"name": "x"}"#).unwrap();
        assert!(pkg.workspaces.is_none());
    }
}
