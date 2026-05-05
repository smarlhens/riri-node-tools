//! `.npmrc` upsert helper shared by `nce` (`engine-strict=true`) and
//! `npd` (`save-exact=true`).

use std::path::Path;

/// Outcome of a [`upsert_npmrc_flag`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpmrcOutcome {
    /// File already contained the line; nothing was written.
    AlreadySet,
    /// File was created or appended with the line.
    Added,
}

/// Ensures `dir/.npmrc` contains the literal `flag` line.
///
/// Behaviour:
///   - missing file → create with `flag\n`
///   - existing content already contains `flag` (substring) → no-op
///   - else → append `flag\n` (with a leading newline if the file did not end with one)
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when reading or writing fails.
pub fn upsert_npmrc_flag(dir: &Path, flag: &str) -> std::io::Result<NpmrcOutcome> {
    let path = dir.join(".npmrc");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains(flag) {
        return Ok(NpmrcOutcome::AlreadySet);
    }
    let new_content = if existing.is_empty() {
        format!("{flag}\n")
    } else if existing.ends_with('\n') {
        format!("{existing}{flag}\n")
    } else {
        format!("{existing}\n{flag}\n")
    };
    std::fs::write(&path, new_content)?;
    Ok(NpmrcOutcome::Added)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_file_when_missing() {
        let tmp = TempDir::new().unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "engine-strict=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "engine-strict=true\n");
    }

    #[test]
    fn no_op_when_flag_already_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "engine-strict=true\n").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "engine-strict=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::AlreadySet);
    }

    #[test]
    fn appends_when_other_lines_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "registry=https://example.com\n").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "save-exact=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "registry=https://example.com\nsave-exact=true\n");
    }

    #[test]
    fn appends_with_newline_when_existing_lacks_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "registry=https://example.com").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "save-exact=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "registry=https://example.com\nsave-exact=true\n");
    }
}
