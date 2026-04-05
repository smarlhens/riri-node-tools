use crate::{LockFileResult, PackageJson, PackageManager};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Errors during lockfile or package.json detection.
#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    #[error("no package.json found starting from {0}")]
    NoPackageJson(PathBuf),
    #[error("no lockfile found starting from {0}")]
    NoLockfile(PathBuf),
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse package.json: {0}")]
    PackageJsonParse(#[from] serde_json::Error),
}

const LOCKFILE_NAMES: &[(&str, PackageManager)] = &[
    ("package-lock.json", PackageManager::Npm),
    ("yarn.lock", PackageManager::Yarn),
    ("pnpm-lock.yaml", PackageManager::Pnpm),
];

/// Detect which lockfile exists by walking up from `start`.
///
/// When multiple lockfiles exist in the same directory, the most recently
/// modified one wins.
///
/// # Errors
///
/// Returns [`DetectError::NoLockfile`] if no supported lockfile is found.
pub fn detect_lockfile(start: &Path) -> Result<LockFileResult, DetectError> {
    let names: Vec<&str> = LOCKFILE_NAMES.iter().map(|(n, _)| *n).collect();
    let matches = riri_find_up::find_up(start, &names);

    if matches.is_empty() {
        return Err(DetectError::NoLockfile(start.into()));
    }

    let winner = most_recently_modified(&matches);

    let file_name = winner
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    let package_manager = LOCKFILE_NAMES
        .iter()
        .find(|(n, _)| *n == file_name)
        .map(|(_, pm)| pm.clone())
        .ok_or_else(|| DetectError::NoLockfile(start.into()))?;

    Ok(LockFileResult {
        path: winner.clone(),
        package_manager,
    })
}

/// Find and parse `package.json` by walking up from `start`.
///
/// # Errors
///
/// Returns [`DetectError`] if no `package.json` is found or it can't be parsed.
pub fn find_package_json(start: &Path) -> Result<(PackageJson, PathBuf), DetectError> {
    let path = riri_find_up::find_up_one(start, "package.json")
        .ok_or_else(|| DetectError::NoPackageJson(start.into()))?;

    let content = std::fs::read_to_string(&path).map_err(|e| DetectError::Io {
        path: path.clone(),
        source: e,
    })?;

    let pkg: PackageJson = serde_json::from_str(&content)?;
    Ok((pkg, path))
}

/// Pick the most recently modified file from a non-empty list.
fn most_recently_modified(files: &[PathBuf]) -> &PathBuf {
    files
        .iter()
        .max_by_key(|f| {
            f.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .expect("files must not be empty")
}
