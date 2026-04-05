//! Generic find-up utility — walk up the directory tree to find files.
//!
//! Equivalent to the JS `empathic/find` or `find-up` packages.

use std::path::{Path, PathBuf};

/// Walk up from `start`, returning all matches in the **first** directory
/// that contains at least one of `names`.
///
/// Stops as soon as a directory contains any match, or when the filesystem
/// root is reached.
#[must_use]
pub fn find_up(start: &Path, names: &[&str]) -> Vec<PathBuf> {
    let mut dir = if start.is_file() {
        start
            .parent()
            .map_or_else(|| start.to_path_buf(), Path::to_path_buf)
    } else {
        start.to_path_buf()
    };

    loop {
        let matches: Vec<PathBuf> = names
            .iter()
            .map(|name| dir.join(name))
            .filter(|p| p.exists())
            .collect();

        if !matches.is_empty() {
            return matches;
        }

        if !dir.pop() {
            return Vec::new();
        }
    }
}

/// Convenience wrapper: find a single file by walking up.
///
/// Returns the path to the first match, or `None` if not found.
#[must_use]
pub fn find_up_one(start: &Path, name: &str) -> Option<PathBuf> {
    find_up(start, &[name]).into_iter().next()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn finds_file_in_start_directory() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();

        let result = find_up(tmp.path(), &["package.json"]);
        assert_eq!(result, vec![tmp.path().join("package.json")]);
    }

    #[test]
    fn finds_file_in_parent_directory() {
        let tmp = TempDir::new().unwrap();
        let child = tmp.path().join("sub");
        fs::create_dir(&child).unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();

        let result = find_up(&child, &["package.json"]);
        assert_eq!(result, vec![tmp.path().join("package.json")]);
    }

    #[test]
    fn finds_multiple_files_in_same_directory() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package-lock.json"), "{}").unwrap();
        fs::write(tmp.path().join("yarn.lock"), "").unwrap();

        let mut result = find_up(tmp.path(), &["package-lock.json", "yarn.lock"]);
        result.sort();
        let mut expected = vec![
            tmp.path().join("package-lock.json"),
            tmp.path().join("yarn.lock"),
        ];
        expected.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn returns_empty_when_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = find_up(tmp.path(), &["nonexistent.txt"]);
        assert!(result.is_empty());
    }

    #[test]
    fn stops_at_first_directory_with_match() {
        let tmp = TempDir::new().unwrap();
        let child = tmp.path().join("sub");
        fs::create_dir(&child).unwrap();
        // File exists in both parent and child
        fs::write(tmp.path().join("file.txt"), "parent").unwrap();
        fs::write(child.join("file.txt"), "child").unwrap();

        let result = find_up(&child, &["file.txt"]);
        assert_eq!(result, vec![child.join("file.txt")]);
    }

    #[test]
    fn find_up_one_returns_single_match() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("target.txt"), "").unwrap();

        assert_eq!(
            find_up_one(tmp.path(), "target.txt"),
            Some(tmp.path().join("target.txt"))
        );
    }

    #[test]
    fn find_up_one_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(find_up_one(tmp.path(), "missing.txt"), None);
    }
}
