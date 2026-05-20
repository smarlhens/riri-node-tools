use crate::members::WorkspaceMember;
use globset::GlobSet;
use riri_common::PackageManager;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_saphyr::Error,
    },
    #[error("invalid workspace pattern `{pattern}`: {source}")]
    Glob {
        pattern: String,
        #[source]
        source: globset::Error,
    },
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct WorkspaceProject {
    pub(crate) root: PathBuf,
    pub(crate) kind: PackageManager,
    pub(crate) globs: GlobSet,
    pub(crate) patterns: Vec<String>,
}

impl WorkspaceProject {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn kind(&self) -> &PackageManager {
        &self.kind
    }

    /// Returns the list of workspace members under this project.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] if a manifest cannot be read or parsed,
    /// or if a workspace glob pattern is invalid.
    #[allow(clippy::result_large_err)]
    pub fn members(&self) -> Result<Vec<WorkspaceMember>, WorkspaceError> {
        crate::members::enumerate(self)
    }
}

#[must_use]
pub fn detect(cwd: &Path) -> Option<WorkspaceProject> {
    let lockfile = riri_common::detect_lockfile(cwd).ok()?;
    let kind = lockfile.package_manager;
    let patterns = match kind {
        PackageManager::Npm | PackageManager::Yarn => read_workspaces_field(cwd)?,
        PackageManager::Pnpm => read_pnpm_yaml(cwd)?,
    };
    if patterns.is_empty() {
        return None;
    }
    let globs = compile_globs(&patterns).ok()?;
    Some(WorkspaceProject {
        root: cwd.to_path_buf(),
        kind,
        globs,
        patterns,
    })
}

fn read_workspaces_field(cwd: &Path) -> Option<Vec<String>> {
    let path = cwd.join("package.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let pkg: riri_common::PackageJson = serde_json::from_str(&raw).ok()?;
    pkg.workspaces.map(|w| w.packages())
}

#[derive(Debug, serde::Deserialize, Default)]
struct PnpmWorkspaceYaml {
    #[serde(default)]
    packages: Vec<String>,
}

fn read_pnpm_yaml(cwd: &Path) -> Option<Vec<String>> {
    let path = cwd.join("pnpm-workspace.yaml");
    let raw = std::fs::read_to_string(&path).ok()?;
    let parsed: PnpmWorkspaceYaml = serde_saphyr::from_str(&raw).ok()?;
    if parsed.packages.is_empty() {
        None
    } else {
        Some(parsed.packages)
    }
}

#[allow(clippy::result_large_err)]
fn compile_globs(patterns: &[String]) -> Result<GlobSet, WorkspaceError> {
    let mut builder = globset::GlobSetBuilder::new();
    for pat in patterns {
        let glob = globset::Glob::new(pat).map_err(|source| WorkspaceError::Glob {
            pattern: pat.clone(),
            source,
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| WorkspaceError::Glob {
        pattern: patterns.join(", "),
        source,
    })
}
