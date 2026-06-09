//! Yarn lockfile and project readers.
//!
//! Two complementary readers, because yarn stores versions but not engines in
//! its lockfile:
//!
//! - [`YarnLock`] parses `yarn.lock` (v1 Classic and v2+ Berry) and resolves a
//!   dependency's locked version by `name@range` descriptor — used by
//!   `riri-npd` for version pinning. No `node_modules` required.
//! - [`YarnProject`] scans `node_modules/<pkg>/package.json` to extract
//!   `engines` (via [`LockfileEngines`]) — used by `riri-nce`. Yarn lockfiles
//!   (any version) do **not** store `engines`, so the install tree is the only
//!   source.

use riri_common::{Engines, LockfileEngines, LockfileVersions};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Errors that can occur when scanning a yarn project.
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
#[derive(Debug, Clone, Deserialize)]
struct NodeModulePackageJson {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    engines: Option<Engines>,
}

/// One scanned `node_modules/<pkg>/package.json` entry.
#[derive(Debug, Clone, Default)]
struct ScannedPackage {
    version: Option<String>,
    engines: Option<Engines>,
}

/// Scanned yarn project with engine + version data from `node_modules`.
#[derive(Debug, Clone)]
pub struct YarnProject {
    packages: HashMap<String, ScannedPackage>,
}

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

impl LockfileEngines for YarnProject {
    fn engines_iter(&self) -> Box<dyn Iterator<Item = (&str, &Engines)> + '_> {
        Box::new(
            self.packages.iter().filter_map(|(name, pkg)| {
                pkg.engines.as_ref().map(|engines| (name.as_str(), engines))
            }),
        )
    }
}

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
/// for berry v2+. Unlike [`YarnProject`] this needs no `node_modules`, works
/// under `PnP`, and distinguishes multiple ranges of the same package.
#[derive(Debug, Clone)]
pub struct YarnLock {
    /// Map of raw descriptor (`foo@^1.2.3` / `foo@npm:^1.2.3`) → resolved version.
    descriptors: HashMap<String, String>,
    /// `true` for berry (v2+) lockfiles, which prefix ranges with `npm:`.
    berry: bool,
}

/// A berry lockfile entry — only the resolved `version` is needed. `version`
/// is captured as a flexible scalar so the `__metadata` block's numeric
/// `version` does not break deserialization of the surrounding map.
#[derive(Debug, Clone, Deserialize)]
struct BerryEntry {
    #[serde(default)]
    version: Option<YamlScalar>,
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
            Ok(Self {
                descriptors: parse_classic(content),
                berry: false,
            })
        } else if content.contains("__metadata") {
            Ok(Self {
                descriptors: parse_berry(content)?,
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
}

/// Parse a classic (v1) `yarn.lock` into a descriptor → version map.
///
/// Entry headers sit at column 0 and end with `:`; they hold one or more
/// comma-separated, optionally-quoted descriptors. The resolved version is the
/// two-space-indented `version "x.y.z"` line of each block.
fn parse_classic(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current: Vec<String> = Vec::new();

    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        if line.starts_with(' ') || line.starts_with('\t') {
            // Entry body — only the top-level `  version "x"` line matters.
            if let Some(rest) = line.strip_prefix("  version ") {
                let version = rest.trim().trim_matches('"');
                for descriptor in &current {
                    map.insert(descriptor.clone(), version.to_string());
                }
            }
        } else {
            // Entry header — comma-joined descriptors ending with `:`.
            let header = line.trim_end().strip_suffix(':').unwrap_or(line);
            current = header
                .split(", ")
                .map(|d| d.trim().trim_matches('"').to_string())
                .collect();
        }
    }

    map
}

/// Parse a berry (v2+) `yarn.lock` (syml/YAML) into a descriptor → version map.
fn parse_berry(content: &str) -> Result<HashMap<String, String>, YarnParseError> {
    let raw: HashMap<String, BerryEntry> = serde_saphyr::from_str(content)?;
    let mut map = HashMap::new();

    for (key, entry) in raw {
        if key == "__metadata" {
            continue;
        }
        let Some(version) = entry.version.map(YamlScalar::into_string) else {
            continue;
        };
        // Berry merges descriptors resolving to the same version under one
        // comma-joined key (e.g. `"a@npm:^1.0.0, a@npm:^1.2.0"`).
        for descriptor in key.split(", ") {
            map.insert(descriptor.trim().to_string(), version.clone());
        }
    }

    Ok(map)
}

impl LockfileVersions for YarnLock {
    /// Best-effort name-only lookup (no range); prefer [`Self::resolved_version`].
    fn version_for(&self, name: &str) -> Option<&str> {
        self.descriptors.iter().find_map(|(descriptor, version)| {
            (descriptor_name(descriptor) == name).then_some(version.as_str())
        })
    }

    fn resolved_version(&self, name: &str, range: &str) -> Option<&str> {
        self.descriptors
            .get(&self.descriptor_for(name, range))
            .map(String::as_str)
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
