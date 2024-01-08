use core::{LockFileResult, PackageManager};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

struct FindUpOptions<'a> {
    pub cwd: &'a Path,
}

impl<'a> Default for FindUpOptions<'a> {
    fn default() -> Self {
        Self {
            cwd: Path::new("."),
        }
    }
}

fn find_up_multiple<T: AsRef<Path>>(
    file_names: &[T],
    options: FindUpOptions,
) -> std::io::Result<Vec<PathBuf>> {
    let cwd_buf = std::env::current_dir().unwrap();
    let cwd = if options.cwd.eq(Path::new(".")) {
        Path::new(&cwd_buf)
    } else {
        options.cwd
    };

    let mut matches = Vec::new();
    let mut target_dir = Some(cwd);
    while let Some(dir) = target_dir {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            for target_file_name in file_names {
                if let Some(file_name) = path.file_name() {
                    if target_file_name.as_ref() == file_name {
                        matches.push(path.clone());
                    }
                }
            }

            if matches.len() > 0 {
                return Ok(matches);
            }

            target_dir = dir.parent();
        }
    }
    Ok(matches)
}

fn get_file_path(path_str: &str) -> Result<PathBuf, Error> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("{:?} file not found!", path_str),
        ));
    }

    Ok(path.to_path_buf())
}

fn find_most_recently_modified(files: Vec<PathBuf>) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }

    let mut most_recent_file = files[0].clone();
    let mut most_recent_time = SystemTime::UNIX_EPOCH;

    for file in files.iter() {
        if let Ok(metadata) = file.metadata() {
            if let Ok(modified_time) = metadata.modified() {
                if modified_time > most_recent_time {
                    most_recent_time = modified_time;
                    most_recent_file = file.clone();
                }
            }
        }
    }

    Some(most_recent_file)
}

pub fn get_package() -> Result<PathBuf, Error> {
    match get_file_path("package.json") {
        Ok(path) => Ok(PathBuf::from(path)),
        Err(_) => Err(Error::new(ErrorKind::NotFound, "Package not found!")),
    }
}

pub fn get_most_recently_modified_lock() -> Result<LockFileResult, Error> {
    let lock_file_names = vec!["package-lock.json", "yarn.lock", "pnpm-lock.yml"];
    let options = FindUpOptions::default();

    if let Ok(matches) = find_up_multiple(&lock_file_names, options) {
        if let Some(most_recent_file) = find_most_recently_modified(matches) {
            let package_manager = match most_recent_file.file_name().and_then(|s| s.to_str()) {
                Some("package-lock.json") => PackageManager::Npm,
                Some("yarn.lock") => PackageManager::Yarn,
                Some("pnpm-lock.yml") => PackageManager::Pnpm,
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

    Err(Error::new(ErrorKind::NotFound, "Package lock not found!"))
}
