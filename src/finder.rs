use crate::types::{LockFileResult, PackageManager};
use anyhow::Result;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn find_up_multiple<T: AsRef<Path>>(file_names: &[T]) -> Result<Vec<PathBuf>> {
    let cwd = std::env::current_dir().expect("Failed to get the current directory!");
    let mut matches = Vec::new();
    let mut target_dir = Some(cwd);
    while let Some(dir) = target_dir.clone() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            for target_file_name in file_names {
                if let Some(file_name) = path.file_name() {
                    if target_file_name.as_ref() == file_name {
                        matches.push(path.clone());
                    }
                }
            }

            if !matches.is_empty() {
                return Ok(matches);
            }

            target_dir = dir.parent().map(Path::to_path_buf);
        }
    }
    Ok(matches)
}

fn get_file_path(path_str: &str) -> Result<PathBuf, Error> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("{path_str:?} file not found!"),
        ));
    }

    Ok(path.to_path_buf())
}

fn find_most_recently_modified(files: &Vec<PathBuf>) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }

    let mut most_recent_file = files[0].clone();
    let mut most_recent_time = SystemTime::UNIX_EPOCH;

    for file in files {
        if let Ok(metadata) = file.metadata() {
            if let Ok(modified_time) = metadata.modified() {
                if modified_time > most_recent_time {
                    most_recent_time = modified_time;
                    most_recent_file.clone_from(file);
                }
            }
        }
    }

    Some(most_recent_file)
}

pub fn get_package() -> Result<PathBuf, Error> {
    match get_file_path("package.json") {
        Ok(path) => Ok(path),
        Err(_) => Err(Error::new(ErrorKind::NotFound, "Package not found!")),
    }
}

const NPM_LOCK_FILE: &str = "package-lock.json";
const YARN_LOCK_FILE: &str = "yarn.lock";
const PNPM_LOCK_FILE: &str = "pnpm-lock.yaml";

pub fn get_most_recently_modified_lock() -> Result<LockFileResult, Error> {
    let lock_file_names = vec![NPM_LOCK_FILE, YARN_LOCK_FILE, PNPM_LOCK_FILE];
    if let Ok(matches) = find_up_multiple(&lock_file_names) {
        if let Some(most_recent_file) = find_most_recently_modified(&matches) {
            let package_manager = match most_recent_file.file_name().and_then(|s| s.to_str()) {
                Some(NPM_LOCK_FILE) => PackageManager::Npm,
                Some(YARN_LOCK_FILE) => PackageManager::Yarn,
                Some(PNPM_LOCK_FILE) => PackageManager::Pnpm,
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Unknown lock file format",
                    ));
                }
            };

            return Ok(LockFileResult {
                path: most_recent_file,
                package_manager,
            });
        }
    }

    Err(Error::new(
        ErrorKind::NotFound,
        "Unable to find any lock file inside the current directory!",
    ))
}
