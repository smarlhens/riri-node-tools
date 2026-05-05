//! Yarn engine constraint extractor.
//!
//! Yarn lockfiles (v1 Classic and v2+ Berry) do **not** store `engines`.
//! Instead, this crate scans `node_modules/<pkg>/package.json` to extract
//! engine constraints, then exposes them via the [`LockfileEngines`] trait.

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
