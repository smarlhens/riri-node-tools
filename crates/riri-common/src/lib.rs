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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

impl PackageManager {
    pub const ALL: [Self; 3] = [Self::Npm, Self::Yarn, Self::Pnpm];

    #[must_use]
    pub const fn lockfile_name(self) -> &'static str {
        match self {
            Self::Npm => "package-lock.json",
            Self::Yarn => "yarn.lock",
            Self::Pnpm => "pnpm-lock.yaml",
        }
    }

    #[must_use]
    pub fn from_lockfile_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pm| pm.lockfile_name() == name)
    }

    /// Parse from a lowercase name (`"npm"`, `"pnpm"`, `"yarn"`).
    #[must_use]
    pub fn from_str_lowercase(name: &str) -> Option<Self> {
        match name {
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            _ => None,
        }
    }
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
    ///
    /// `package_name` is the bare package name — `typescript`, `@scope/pkg` —
    /// never the lockfile's own key. Implementations strip whatever decoration
    /// their format uses: npm's `node_modules/` path prefixes, pnpm's
    /// `@version` and peer suffixes. The npm root entry keeps its empty key.
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

    /// Resolves the locked version of a pnpm catalog entry: `catalog` is
    /// `None` for the default catalog, `Some(name)` for a named one.
    fn catalog_version(&self, _catalog: Option<&str>, name: &str) -> Option<&str> {
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

/// Bare package name from a `node_modules` path key.
#[must_use]
pub fn bare_package_name(key: &str) -> &str {
    // Not `rfind("node_modules/")`: a multi-byte needle rebuilds a two-way
    // searcher per call, and this runs once per lockfile entry.
    let bytes = key.as_bytes();
    let mut end = bytes.len();
    while let Some(slash) = bytes[..end].iter().rposition(|&b| b == b'/') {
        if bytes[..slash].ends_with(b"node_modules") {
            return &key[slash + 1..];
        }
        end = slash;
    }
    key
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

/// Returns `true` if npm resolves the specifier from the registry: a version, a
/// range or a dist-tag (npm-package-arg's `registry` types). These only use
/// [`is_registry_byte`]s, so a URL, git spec, alias, path or protocol (`:`, `/`,
/// `@`, `#`) is rejected without being listed.
#[must_use]
pub fn is_registry_specifier(spec: &str) -> bool {
    spec.bytes().all(is_registry_byte)
        && (!spec.bytes().any(|b| b == b' ' || b == b'\t') || starts_like_range(spec))
        && !spec.starts_with('.')
        && !is_tarball_name(spec.as_bytes())
        && spec != "catalog"
}

/// Range syntax, plus the bytes a dist-tag may use (URI-safe ones).
const fn is_registry_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'.' | b'_'
                | b'-'
                | b'*'
                | b'^'
                | b'~'
                | b'<'
                | b'>'
                | b'='
                | b'|'
                | b'+'
                | b'!'
                | b'\''
                | b'('
                | b')'
                | b' '
                | b'\t'
        )
}

/// Whitespace only belongs inside a range, never in a dist-tag.
fn starts_like_range(spec: &str) -> bool {
    matches!(
        spec.trim_ascii_start().as_bytes().first(),
        Some(b'0'..=b'9' | b'v' | b'=' | b'<' | b'>' | b'~' | b'^' | b'*' | b'x' | b'X')
    )
}

/// npm-package-arg's `/[.](?:tgz|tar.gz|tar)$/i`, which makes the spec a file
/// path. Its unescaped `.` in `tar.gz` matches any byte.
fn is_tarball_name(spec: &[u8]) -> bool {
    let tail = |n: usize| spec.len().checked_sub(n).map(|start| &spec[start..]);
    tail(4).is_some_and(|t| t.eq_ignore_ascii_case(b".tgz") || t.eq_ignore_ascii_case(b".tar"))
        || tail(7).is_some_and(|t| {
            t[..4].eq_ignore_ascii_case(b".tar") && t[5..].eq_ignore_ascii_case(b"gz")
        })
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
    fn bare_package_name_strips_node_modules_segments() {
        assert_eq!(bare_package_name("node_modules/typescript"), "typescript");
        assert_eq!(bare_package_name("node_modules/@scope/pkg"), "@scope/pkg");
        assert_eq!(bare_package_name("node_modules/a/node_modules/b"), "b");
        assert_eq!(
            bare_package_name("node_modules/a/node_modules/@scope/b"),
            "@scope/b"
        );
        // Keys that are not node_modules paths pass through untouched.
        assert_eq!(bare_package_name(""), "");
        assert_eq!(bare_package_name("typescript"), "typescript");
        assert_eq!(bare_package_name("packages/app"), "packages/app");
    }

    #[test]
    fn package_manager_round_trips_names() {
        for pm in PackageManager::ALL {
            assert_eq!(
                PackageManager::from_lockfile_name(pm.lockfile_name()),
                Some(pm)
            );
        }
        assert_eq!(
            PackageManager::from_str_lowercase("pnpm"),
            Some(PackageManager::Pnpm)
        );
        assert!(PackageManager::from_str_lowercase("bun").is_none());
        assert!(PackageManager::from_lockfile_name("bun.lockb").is_none());
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

    #[test]
    fn registry_specifiers_are_versions_ranges_and_tags_only() {
        for spec in [
            "1.2.3",
            "^1.2.3",
            ">=1 <2 || ^3",
            "1.x",
            "latest",
            "my_tag!(v2)'",
        ] {
            assert!(is_registry_specifier(spec), "{spec}");
        }
        for spec in [
            "https://cdn.sheetjs.com/xlsx-0.20.3/xlsx-0.20.3.tgz",
            "github:user/repo#v2",
            "user/repo",
            "npm:bar@^1.0.0",
            "workspace:*",
            "catalog",
            "..",
            "pkg.TGZ",
            "pkg.tar",
            "pkg.tar_gz",
            "foo bar",
            "latest\tnext",
        ] {
            assert!(!is_registry_specifier(spec), "{spec}");
        }
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
