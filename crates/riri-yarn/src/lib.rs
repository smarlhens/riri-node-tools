//! Yarn lockfile and project readers.
//!
//! Two complementary readers, because yarn stores versions but not engines in
//! its lockfile:
//!
//! - [`YarnLock`] parses `yarn.lock` (v1 Classic and v2+ Berry) and resolves a
//!   dependency's locked version by `name@range` descriptor — used by
//!   `riri-npd` for version pinning. No `node_modules` required.
//! - `YarnProject` (feature `scan`) reads `node_modules/<pkg>/package.json` to extract
//!   `engines` (via [`LockfileEngines`]) — used by `riri-nce`. Yarn lockfiles
//!   (any version) do **not** store `engines`, so the install tree is the only
//!   source.

use riri_common::LockfileVersions;
#[cfg(feature = "scan")]
use riri_common::{Engines, LockfileEngines};
#[cfg(feature = "graph")]
use riri_common::{
    GraphError, LockGraph, LockGraphEdge, LockGraphNode, LockGraphRoot, LockfileGraph, PackageJson,
    RootDepKind, is_local_specifier,
};
use serde::Deserialize;
use std::collections::HashMap;
#[cfg(feature = "scan")]
use std::path::Path;

/// Errors that can occur when scanning a yarn project.
#[cfg(feature = "scan")]
#[derive(Debug, thiserror::Error)]
pub enum YarnScanError {
    #[error("node_modules directory not found at {0}")]
    NodeModulesNotFound(std::path::PathBuf),
    #[error("failed to read {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Json {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
}

/// Minimal package.json representation for engine + version extraction.
#[cfg(feature = "scan")]
#[derive(Debug, Clone, Deserialize)]
struct NodeModulePackageJson {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    engines: Option<Engines>,
}

/// One scanned `node_modules/<pkg>/package.json` entry.
#[cfg(feature = "scan")]
#[derive(Debug, Clone, Default)]
struct ScannedPackage {
    version: Option<String>,
    engines: Option<Engines>,
}

/// Scanned yarn project with engine + version data from `node_modules`.
#[cfg(feature = "scan")]
#[derive(Debug, Clone)]
pub struct YarnProject {
    packages: HashMap<String, ScannedPackage>,
}

#[cfg(feature = "scan")]
impl YarnProject {
    /// Scan `node_modules/` under the given project directory to extract engine
    /// constraints from each installed package's `package.json`.
    ///
    /// Walks at depth 1-2 to handle both regular packages (`node_modules/foo`)
    /// and scoped packages (`node_modules/@scope/bar`).
    ///
    /// # Errors
    ///
    /// Returns [`YarnScanError::NodeModulesNotFound`] if the `node_modules`
    /// directory does not exist (e.g. `PnP` mode or deps not installed).
    ///
    /// # Panics
    ///
    /// Panics if a walked entry cannot be stripped of the `node_modules`
    /// prefix, which should never happen since all entries originate from
    /// that directory.
    pub fn scan(project_dir: &Path) -> Result<Self, YarnScanError> {
        let node_modules = project_dir.join("node_modules");
        if !node_modules.is_dir() {
            return Err(YarnScanError::NodeModulesNotFound(node_modules));
        }

        let mut packages = HashMap::new();

        for entry in walkdir::WalkDir::new(&node_modules)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories (like .cache, .package-lock.json)
                e.file_name()
                    .to_str()
                    .is_some_and(|name| !name.starts_with('.'))
            })
        {
            let Ok(entry) = entry else { continue };

            if !entry.file_type().is_dir() {
                continue;
            }

            let pkg_json_path = entry.path().join("package.json");
            if !pkg_json_path.exists() {
                continue;
            }

            // Extract package name from path relative to node_modules.
            let rel_path = entry
                .path()
                .strip_prefix(&node_modules)
                .expect("entry is under node_modules");
            let name = rel_path.to_string_lossy().replace('\\', "/");

            // Skip scope directories themselves (@scope without /pkg).
            if name.starts_with('@') && !name.contains('/') {
                continue;
            }

            let content =
                std::fs::read_to_string(&pkg_json_path).map_err(|e| YarnScanError::Io {
                    path: pkg_json_path.clone(),
                    source: e,
                })?;

            let pkg: NodeModulePackageJson =
                serde_json::from_str(&content).map_err(|e| YarnScanError::Json {
                    path: pkg_json_path,
                    source: e,
                })?;

            packages.insert(
                name,
                ScannedPackage {
                    version: pkg.version,
                    engines: pkg.engines,
                },
            );
        }

        Ok(Self { packages })
    }
}

#[cfg(feature = "scan")]
impl LockfileEngines for YarnProject {
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_> {
        Box::new(
            self.packages.iter().filter_map(|(name, pkg)| {
                pkg.engines.as_ref().map(|engines| (name.as_str(), engines))
            }),
        )
    }
}

#[cfg(feature = "scan")]
impl LockfileVersions for YarnProject {
    fn version_for(&self, name: &str) -> Option<&str> {
        self.packages.get(name)?.version.as_deref()
    }
}

/// Errors that can occur when parsing a `yarn.lock` file.
#[derive(Debug, thiserror::Error)]
pub enum YarnParseError {
    #[error(
        "unrecognized yarn.lock format (missing '# yarn lockfile v1' header and '__metadata' block)"
    )]
    UnknownFormat,
    #[error("failed to parse berry yarn.lock as YAML: {0}")]
    Yaml(#[from] serde_saphyr::Error),
}

/// A parsed `yarn.lock`, keyed by the `name@range` descriptor exactly as the
/// lockfile records it.
///
/// Resolution mirrors the legacy JS `@smarlhens/npm-pin-dependencies`: a
/// dependency's locked version is looked up by the descriptor built from its
/// `package.json` specifier — `name@range` for classic v1, `name@npm:range`
/// for berry v2+. Unlike `YarnProject` this needs no `node_modules`, works
/// under `PnP`, and distinguishes multiple ranges of the same package.
#[derive(Debug, Clone)]
pub struct YarnLock {
    /// One entry per resolved package instance (parse order).
    entries: Vec<YarnEntry>,
    /// Raw descriptor (`foo@^1.2.3` / `foo@npm:^1.2.3`) → index into `entries`.
    descriptor_index: HashMap<String, usize>,
    /// `true` for berry (v2+) lockfiles, which prefix ranges with `npm:`.
    berry: bool,
}

/// One resolved `yarn.lock` entry: name, version, and declared deps.
#[derive(Debug, Clone)]
struct YarnEntry {
    name: String,
    version: String,
    /// Declared deps as `(name, range)` — the `dependencies` block. Only read by
    /// the `graph`-gated `LockfileGraph` impl.
    #[cfg_attr(not(feature = "graph"), allow(dead_code))]
    deps: Vec<(String, String)>,
}

/// A berry lockfile entry. `version` is a flexible scalar so the `__metadata`
/// block's numeric `version` does not break deserialization. `dependencies`
/// maps dep name → range (range may carry a protocol like `npm:` / `workspace:`).
#[derive(Debug, Clone, Deserialize)]
struct BerryEntry {
    #[serde(default)]
    version: Option<YamlScalar>,
    #[serde(default)]
    dependencies: Option<HashMap<String, YamlScalar>>,
}

/// A YAML scalar that may be a string or number — yarn package versions are
/// strings, but `__metadata.version` is an integer.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum YamlScalar {
    Str(String),
    Int(i64),
    Float(f64),
}

impl YamlScalar {
    fn into_string(self) -> String {
        match self {
            Self::Str(s) => s,
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
        }
    }
}

impl YarnLock {
    /// Parse a `yarn.lock` file, auto-detecting classic v1 vs berry v2+.
    ///
    /// # Errors
    ///
    /// Returns [`YarnParseError::UnknownFormat`] when neither the classic
    /// header nor the berry `__metadata` block is present, or
    /// [`YarnParseError::Yaml`] when a berry lockfile is not valid YAML.
    pub fn parse(content: &str) -> Result<Self, YarnParseError> {
        if content.contains("# yarn lockfile v1") {
            let (entries, descriptor_index) = parse_classic(content);
            Ok(Self {
                entries,
                descriptor_index,
                berry: false,
            })
        } else if content.contains("__metadata") {
            let (entries, descriptor_index) = parse_berry(content)?;
            Ok(Self {
                entries,
                descriptor_index,
                berry: true,
            })
        } else {
            Err(YarnParseError::UnknownFormat)
        }
    }

    /// Build the lockfile descriptor for a `package.json` `name`/`range` pair.
    fn descriptor_for(&self, name: &str, range: &str) -> String {
        if self.berry {
            format!("{name}@npm:{range}")
        } else {
            format!("{name}@{range}")
        }
    }

    /// Resolve a `(name, range)` dependency to an entry index. Berry prefixes
    /// plain semver ranges with `npm:`; ranges already carrying a protocol
    /// (`npm:`, `workspace:`, `patch:` …) are tried verbatim. Local protocols
    /// never resolve.
    #[cfg(feature = "graph")]
    fn lookup(&self, name: &str, range: &str) -> Option<usize> {
        if is_local_specifier(range) || range.starts_with("patch:") || range.starts_with("portal:")
        {
            return None;
        }
        let descriptor = if self.berry && !range.contains(':') {
            format!("{name}@npm:{range}")
        } else {
            format!("{name}@{range}")
        };
        self.descriptor_index.get(&descriptor).copied()
    }
}

/// Parse a classic (v1) `yarn.lock` into entries + a descriptor → index map.
///
/// Entry headers sit at column 0 and end with `:`; they hold one or more
/// comma-separated, optionally-quoted descriptors. Two-space-indented body
/// lines carry `version "x.y.z"`; a `  dependencies:` line opens a section of
/// four-space-indented `    name "range"` declarations.
fn parse_classic(content: &str) -> (Vec<YarnEntry>, HashMap<String, usize>) {
    let mut entries: Vec<YarnEntry> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut descriptors: Vec<String> = Vec::new();
    let mut version: Option<String> = None;
    let mut deps: Vec<(String, String)> = Vec::new();
    let mut in_deps = false;

    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(rest) = line.strip_prefix("  version ") {
                version = Some(rest.trim().trim_matches('"').to_string());
                in_deps = false;
            } else if line.trim_end() == "  dependencies:" {
                in_deps = true;
            } else if in_deps && line.starts_with("    ") {
                if let Some(dep) = parse_classic_dep_line(line.trim()) {
                    deps.push(dep);
                }
            } else {
                // Any other field (`resolved`, `optionalDependencies:` …) ends
                // the dependencies section.
                in_deps = false;
            }
        } else {
            flush_classic_entry(
                &mut entries,
                &mut index,
                &mut descriptors,
                &mut version,
                &mut deps,
            );
            in_deps = false;
            let header = line.trim_end().strip_suffix(':').unwrap_or(line);
            descriptors = header
                .split(", ")
                .map(|d| d.trim().trim_matches('"').to_string())
                .collect();
        }
    }
    flush_classic_entry(
        &mut entries,
        &mut index,
        &mut descriptors,
        &mut version,
        &mut deps,
    );

    (entries, index)
}

/// Push the pending classic entry (if it has a version) and reset the buffers.
fn flush_classic_entry(
    entries: &mut Vec<YarnEntry>,
    index: &mut HashMap<String, usize>,
    descriptors: &mut Vec<String>,
    version: &mut Option<String>,
    deps: &mut Vec<(String, String)>,
) {
    if let (Some(version), Some(first)) = (version.take(), descriptors.first()) {
        let idx = entries.len();
        entries.push(YarnEntry {
            name: descriptor_name(first).to_string(),
            version,
            deps: std::mem::take(deps),
        });
        for descriptor in descriptors.iter() {
            index.insert(descriptor.clone(), idx);
        }
    }
    descriptors.clear();
    deps.clear();
}

/// Parse a classic `    name "range"` / `    "@scope/name" "range"` line.
fn parse_classic_dep_line(line: &str) -> Option<(String, String)> {
    let (name, range) = line.split_once(' ')?;
    Some((
        name.trim().trim_matches('"').to_string(),
        range.trim().trim_matches('"').to_string(),
    ))
}

/// Parse a berry (v2+) `yarn.lock` (syml/YAML) into entries + descriptor index.
fn parse_berry(content: &str) -> Result<(Vec<YarnEntry>, HashMap<String, usize>), YarnParseError> {
    let raw: HashMap<String, BerryEntry> = serde_saphyr::from_str(content)?;
    let mut entries: Vec<YarnEntry> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    // Sorted keys for deterministic entry order (HashMap iteration is random).
    let mut keys: Vec<&String> = raw.keys().collect();
    keys.sort();

    for key in keys {
        if key == "__metadata" {
            continue;
        }
        let entry = &raw[key];
        let Some(version) = entry.version.clone().map(YamlScalar::into_string) else {
            continue;
        };
        let mut deps: Vec<(String, String)> = entry
            .dependencies
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(n, r)| (n.clone(), r.clone().into_string()))
                    .collect()
            })
            .unwrap_or_default();
        deps.sort();

        let idx = entries.len();
        let first = key.split(", ").next().unwrap_or(key).trim();
        entries.push(YarnEntry {
            name: descriptor_name(first).to_string(),
            version,
            deps,
        });
        // Berry merges descriptors resolving to the same version under one
        // comma-joined key (e.g. `"a@npm:^1.0.0, a@npm:^1.2.0"`).
        for descriptor in key.split(", ") {
            index.insert(descriptor.trim().to_string(), idx);
        }
    }

    Ok((entries, index))
}

impl LockfileVersions for YarnLock {
    /// Best-effort name-only lookup (no range); prefer [`Self::resolved_version`].
    fn version_for(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.version.as_str())
    }

    fn resolved_version(&self, name: &str, range: &str) -> Option<&str> {
        let idx = self
            .descriptor_index
            .get(&self.descriptor_for(name, range))?;
        Some(self.entries[*idx].version.as_str())
    }
}

#[cfg(feature = "graph")]
impl LockfileGraph for YarnLock {
    fn dep_graph(&self, package_json: &PackageJson) -> Result<LockGraph, GraphError> {
        let mut graph = LockGraph::default();
        // entries → nodes 1:1 (same indices).
        for entry in &self.entries {
            graph.nodes.push(LockGraphNode {
                name: entry.name.clone(),
                version: entry.version.clone(),
                deps: Vec::new(),
            });
        }
        for (i, entry) in self.entries.iter().enumerate() {
            let mut deps = entry.deps.clone();
            deps.sort();
            for (name, range) in deps {
                if let Some(target) = self.lookup(&name, &range) {
                    graph.nodes[i].deps.push(LockGraphEdge {
                        node: target,
                        optional: false,
                    });
                }
            }
        }
        let buckets = [
            (RootDepKind::Dependencies, &package_json.dependencies),
            (RootDepKind::DevDependencies, &package_json.dev_dependencies),
            (
                RootDepKind::OptionalDependencies,
                &package_json.optional_dependencies,
            ),
        ];
        for (kind, bucket) in buckets {
            let Some(bucket) = bucket else { continue };
            let mut entries: Vec<(&String, &String)> = bucket.iter().collect();
            entries.sort();
            for (name, range) in entries {
                if is_local_specifier(range) {
                    continue;
                }
                if let Some(node) = self.lookup(name, range) {
                    graph.roots.push(LockGraphRoot {
                        kind,
                        range: range.clone(),
                        node,
                    });
                }
            }
        }
        Ok(graph)
    }
}

/// Extract the package name from a descriptor (`foo@^1` → `foo`,
/// `@scope/bar@npm:^1` → `@scope/bar`), splitting on the `@` that separates
/// the name from the range (the second `@` for scoped packages).
fn descriptor_name(descriptor: &str) -> &str {
    let search_from = usize::from(descriptor.starts_with('@'));
    match descriptor[search_from..].find('@') {
        Some(at) => &descriptor[..search_from + at],
        None => descriptor,
    }
}

#[cfg(test)]
#[cfg(feature = "graph")]
#[allow(clippy::unwrap_used)]
mod graph_tests {
    use super::*;

    fn pkg(json: &str) -> PackageJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn berry_graph_resolves_transitive_edges() {
        let lock = YarnLock::parse(concat!(
            "__metadata:\n  version: 8\n\n",
            "\"a@npm:^1.0.0\":\n  version: 1.2.0\n  dependencies:\n    b: \"npm:^2.0.0\"\n\n",
            "\"b@npm:^2.0.0\":\n  version: 2.3.0\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!(a.version, "1.2.0");
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.3.0"));
    }

    #[test]
    fn classic_graph_parses_block_dependencies() {
        let lock = YarnLock::parse(concat!(
            "# yarn lockfile v1\n\n",
            "a@^1.0.0:\n  version \"1.2.0\"\n  dependencies:\n    b \"^2.0.0\"\n\n",
            "b@^2.0.0:\n  version \"2.3.0\"\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!(b.version, "2.3.0");
    }

    #[test]
    fn classic_graph_scoped_block_dependency() {
        let lock = YarnLock::parse(concat!(
            "# yarn lockfile v1\n\n",
            "a@^1.0.0:\n  version \"1.2.0\"\n  dependencies:\n    \"@scope/c\" \"^1.0.0\"\n\n",
            "\"@scope/c@^1.0.0\":\n  version \"1.5.0\"\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        let c = &graph.nodes[a.deps[0].node];
        assert_eq!((c.name.as_str(), c.version.as_str()), ("@scope/c", "1.5.0"));
    }

    #[test]
    fn berry_workspace_protocol_dep_skipped() {
        let lock = YarnLock::parse(concat!(
            "__metadata:\n  version: 8\n\n",
            "\"a@npm:^1.0.0\":\n  version: 1.2.0\n  dependencies:\n    b: \"workspace:^\"\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert!(a.deps.is_empty());
    }

    #[test]
    fn unresolvable_descriptor_skipped() {
        let lock = YarnLock::parse(concat!(
            "__metadata:\n  version: 8\n\n",
            "\"a@npm:^1.0.0\":\n  version: 1.2.0\n  dependencies:\n    ghost: \"npm:^9.0.0\"\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert!(a.deps.is_empty());
    }

    #[test]
    fn merged_descriptor_key_is_single_node() {
        let lock = YarnLock::parse(concat!(
            "__metadata:\n  version: 8\n\n",
            "\"a@npm:^1.0.0, a@npm:^1.2.0\":\n  version: 1.3.0\n",
        ))
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.nodes.len(), 1);
        // Both descriptors resolve to the same node.
        assert_eq!(lock.lookup("a", "^1.0.0"), lock.lookup("a", "^1.2.0"));
        assert_eq!(graph.nodes[graph.roots[0].node].version, "1.3.0");
    }
}
