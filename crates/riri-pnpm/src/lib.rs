//! pnpm `pnpm-lock.yaml` parser (v5, v6, v7, v8, v9, v10, v11).
//!
//! Parses the lockfile and exposes engine constraints per dependency
//! via the [`LockfileEngines`] trait.

pub mod catalog;

use riri_common::{Engines, LockfileEngines, LockfileVersions};
#[cfg(feature = "graph")]
use riri_common::{
    GraphError, LockGraph, LockGraphEdge, LockGraphNode, LockGraphRoot, LockfileGraph, PackageJson,
    RootDepKind,
};
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

/// A single package entry in a pnpm lockfile. v5/v6 store deps here; v9 keeps
/// engines here but moves resolved deps to `snapshots`.
#[derive(Debug, Clone, Deserialize)]
pub struct PnpmPackageEntry {
    #[serde(default)]
    pub engines: Option<Engines>,
    #[cfg(feature = "graph")]
    #[serde(default)]
    pub dependencies: Option<HashMap<String, String>>,
    #[cfg(feature = "graph")]
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: Option<HashMap<String, String>>,
}

/// A v9 `snapshots` entry — per-peer-context resolved deps.
#[cfg(feature = "graph")]
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PnpmSnapshotEntry {
    #[serde(default)]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: Option<HashMap<String, String>>,
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
    /// `snapshots` holds per-peer-context resolved deps (the graph source).
    V9 {
        packages: HashMap<String, PnpmPackageEntry>,
        importers: HashMap<String, PnpmImporter>,
        #[cfg(feature = "graph")]
        snapshots: HashMap<String, PnpmSnapshotEntry>,
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
        strip_peer_suffix(self.raw_version())
    }

    /// Returns the raw resolved version, peer suffix included.
    #[must_use]
    pub fn raw_version(&self) -> &str {
        match self {
            Self::Version(v) | Self::Object { version: v, .. } => v.as_str(),
        }
    }

    /// Returns the `package.json` specifier (range), when present (v6+ object form).
    #[cfg(feature = "graph")]
    #[must_use]
    pub fn specifier(&self) -> Option<&str> {
        match self {
            Self::Object { specifier, .. } => specifier.as_deref(),
            Self::Version(_) => None,
        }
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
                    #[cfg(feature = "graph")]
                    snapshots: lock.snapshots,
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
        self.importers().get(".")
    }

    /// All importers (root `.` plus workspace members).
    #[must_use]
    pub fn importers(&self) -> &HashMap<String, PnpmImporter> {
        match self {
            Self::V5 { importers, .. }
            | Self::V6 { importers, .. }
            | Self::V9 { importers, .. } => importers,
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

/// Split a `packages`/`snapshots` key into `(name, version-with-peer-suffix)`.
///
/// - v5: `/name/1.2.3`, `/@scope/name/1.2.3`
/// - v6: `/name@1.2.3(peer)`, `/@scope/name@1.2.3`
/// - v9: `name@1.2.3(peer)`, `@scope/name@1.2.3`
#[cfg(feature = "graph")]
fn parse_pkg_key(key: &str) -> Option<(String, String)> {
    let k = key.strip_prefix('/').unwrap_or(key);
    if let Some(idx) = find_name_version_at(k) {
        return Some((k[..idx].to_string(), k[idx + 1..].to_string()));
    }
    // v5 slash form: version is the segment after the last `/`.
    let (name, version) = k.rsplit_once('/')?;
    Some((name.to_string(), version.to_string()))
}

/// Index of the `@` separating name from version: ignores a leading scope `@`
/// and only looks before any `(` peer suffix.
#[cfg(feature = "graph")]
fn find_name_version_at(k: &str) -> Option<usize> {
    let stop = k.find('(').unwrap_or(k.len());
    k[..stop].rfind('@').filter(|&i| i > 0)
}

/// Strip the pnpm peer-context suffix: `1.2.3(react@18.2.0)` → `1.2.3`.
fn strip_peer_suffix(v: &str) -> &str {
    v.split_once('(').map_or(v, |(s, _)| s)
}

/// A node-source entry: `(key, dependencies, optionalDependencies)`.
#[cfg(feature = "graph")]
type GraphEntry<'a> = (
    &'a String,
    &'a Option<HashMap<String, String>>,
    &'a Option<HashMap<String, String>>,
);

#[cfg(feature = "graph")]
impl PnpmLockfile {
    /// Node-source entries: v9 → `snapshots`, v5/v6 → `packages`, each yielding
    /// `(key, dependencies, optionalDependencies)`.
    fn graph_entries(&self) -> Vec<GraphEntry<'_>> {
        match self {
            Self::V9 { snapshots, .. } => snapshots
                .iter()
                .map(|(k, e)| (k, &e.dependencies, &e.optional_dependencies))
                .collect(),
            Self::V5 { packages, .. } | Self::V6 { packages, .. } => packages
                .iter()
                .map(|(k, e)| (k, &e.dependencies, &e.optional_dependencies))
                .collect(),
        }
    }

    /// Build the lookup key for a dependency `value` in this lockfile's format.
    fn dep_key(&self, name: &str, value: &str) -> String {
        match self {
            Self::V9 { .. } => format!("{name}@{value}"),
            Self::V6 { .. } => format!("/{name}@{value}"),
            Self::V5 { .. } => format!("/{name}/{value}"),
        }
    }

    /// Resolve a dependency `(name, value)` to a node index, retrying with the
    /// peer suffix stripped on a miss.
    fn resolve_dep(
        &self,
        name: &str,
        value: &str,
        key_to_idx: &HashMap<String, usize>,
    ) -> Option<usize> {
        if let Some(&i) = key_to_idx.get(&self.dep_key(name, value)) {
            return Some(i);
        }
        let stripped = strip_peer_suffix(value);
        if stripped != value
            && let Some(&i) = key_to_idx.get(&self.dep_key(name, stripped))
        {
            return Some(i);
        }
        None
    }
}

#[cfg(feature = "graph")]
impl LockfileGraph for PnpmLockfile {
    fn dep_graph(&self, _package_json: &PackageJson) -> Result<LockGraph, GraphError> {
        let mut graph = LockGraph::default();
        let mut key_to_idx: HashMap<String, usize> = HashMap::new();

        let mut entries = self.graph_entries();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        // Phase 1: one node per parseable key.
        for (key, _, _) in &entries {
            if let Some((name, raw_version)) = parse_pkg_key(key) {
                let idx = graph.nodes.len();
                graph.nodes.push(LockGraphNode {
                    name,
                    version: strip_peer_suffix(&raw_version).to_string(),
                    deps: Vec::new(),
                });
                key_to_idx.insert((*key).clone(), idx);
            }
        }

        // Phase 2: edges from `dependencies` + `optionalDependencies`.
        for (key, deps, opt) in &entries {
            let Some(&from) = key_to_idx.get(*key) else {
                continue;
            };
            let mut edges: Vec<(String, String, bool)> = Vec::new();
            if let Some(d) = deps {
                edges.extend(d.iter().map(|(n, v)| (n.clone(), v.clone(), false)));
            }
            if let Some(d) = opt {
                edges.extend(d.iter().map(|(n, v)| (n.clone(), v.clone(), true)));
            }
            edges.sort();
            for (name, value, optional) in edges {
                if value.starts_with("link:") || value.starts_with("file:") {
                    continue;
                }
                if let Some(node) = self.resolve_dep(&name, &value, &key_to_idx) {
                    graph.nodes[from]
                        .deps
                        .push(LockGraphEdge { node, optional });
                }
            }
        }

        // Roots: every importer (root + workspace members), every bucket.
        let importers = self.importers();
        let mut importer_keys: Vec<&String> = importers.keys().collect();
        importer_keys.sort();
        for ik in importer_keys {
            let importer = &importers[ik];
            let buckets = [
                (RootDepKind::Dependencies, &importer.dependencies),
                (RootDepKind::DevDependencies, &importer.dev_dependencies),
                (
                    RootDepKind::OptionalDependencies,
                    &importer.optional_dependencies,
                ),
            ];
            for (kind, bucket) in buckets {
                let mut names: Vec<&String> = bucket.keys().collect();
                names.sort();
                for name in names {
                    let dep = &bucket[name];
                    let raw = dep.raw_version();
                    if raw.starts_with("link:") || raw.starts_with("file:") {
                        continue;
                    }
                    let range = dep.specifier().unwrap_or("*").to_string();
                    if let Some(node) = self.resolve_dep(name, raw, &key_to_idx) {
                        graph.roots.push(LockGraphRoot { kind, range, node });
                    }
                }
            }
        }

        Ok(graph)
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
    #[cfg(feature = "graph")]
    #[serde(default)]
    snapshots: HashMap<String, PnpmSnapshotEntry>,
}

#[cfg(test)]
#[cfg(feature = "graph")]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use super::*;

    /// pnpm `dep_graph` ignores `package_json` (roots come from importers).
    fn empty_pkg() -> PackageJson {
        serde_saphyr::from_str("{}").unwrap()
    }

    #[test]
    fn v9_resolves_snapshot_edges_with_peer_suffix() {
        let lock = PnpmLockfile::parse(
            "
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      a:
        specifier: ^1.0.0
        version: 1.2.0
packages:
  a@1.2.0: {}
  b@2.0.0: {}
snapshots:
  a@1.2.0:
    dependencies:
      b: 2.0.0(c@1.0.0)
  b@2.0.0(c@1.0.0): {}
",
        )
        .unwrap();
        let graph = lock.dep_graph(&empty_pkg()).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.2.0"));
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.0.0"));
        assert_eq!(graph.roots[0].range, "^1.0.0");
    }

    #[test]
    fn v9_optional_dependency_flagged() {
        let lock = PnpmLockfile::parse(
            "
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      a:
        specifier: ^1.0.0
        version: 1.2.0
packages: {}
snapshots:
  a@1.2.0:
    optionalDependencies:
      b: 2.0.0
  b@2.0.0: {}
",
        )
        .unwrap();
        let graph = lock.dep_graph(&empty_pkg()).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!(a.deps.len(), 1);
        assert!(a.deps[0].optional);
        assert_eq!(graph.nodes[a.deps[0].node].name, "b");
    }

    #[test]
    fn v9_workspace_importer_roots_and_link_skipped() {
        let lock = PnpmLockfile::parse(
            "
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      a:
        specifier: ^1.0.0
        version: 1.2.0
  packages/app:
    dependencies:
      b:
        specifier: ^2.0.0
        version: 2.0.0
      local:
        specifier: workspace:*
        version: link:../local
packages: {}
snapshots:
  a@1.2.0: {}
  b@2.0.0: {}
",
        )
        .unwrap();
        let graph = lock.dep_graph(&empty_pkg()).unwrap();
        // a from root importer, b from workspace importer; local (link:) skipped.
        assert_eq!(graph.roots.len(), 2);
        let names: Vec<&str> = graph
            .roots
            .iter()
            .map(|r| graph.nodes[r.node].name.as_str())
            .collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn v6_slash_at_keys_with_package_deps() {
        let lock = PnpmLockfile::parse(
            "
lockfileVersion: '6.0'
importers:
  .:
    dependencies:
      a:
        specifier: ^1.0.0
        version: 1.2.0
packages:
  /a@1.2.0:
    dependencies:
      b: 2.0.0
  /b@2.0.0: {}
",
        )
        .unwrap();
        let graph = lock.dep_graph(&empty_pkg()).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.2.0"));
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.0.0"));
    }

    #[test]
    fn v5_slash_keys_and_wildcard_range() {
        let lock = PnpmLockfile::parse(
            "
lockfileVersion: 5.4
importers:
  .:
    dependencies:
      a: 1.2.0
packages:
  /a/1.2.0:
    dependencies:
      b: 2.0.0
  /b/2.0.0: {}
",
        )
        .unwrap();
        let graph = lock.dep_graph(&empty_pkg()).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.2.0"));
        // v5 importer Version form carries no specifier → wildcard range.
        assert_eq!(graph.roots[0].range, "*");
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.0.0"));
    }

    #[test]
    fn parse_pkg_key_handles_all_forms() {
        assert_eq!(
            parse_pkg_key("/a/1.2.0"),
            Some(("a".into(), "1.2.0".into()))
        );
        assert_eq!(
            parse_pkg_key("/@scope/name/1.2.0"),
            Some(("@scope/name".into(), "1.2.0".into()))
        );
        assert_eq!(
            parse_pkg_key("/a@1.2.0(peer@1.0.0)"),
            Some(("a".into(), "1.2.0(peer@1.0.0)".into()))
        );
        assert_eq!(
            parse_pkg_key("@scope/name@1.2.0"),
            Some(("@scope/name".into(), "1.2.0".into()))
        );
    }
}
