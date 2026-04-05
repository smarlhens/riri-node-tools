use crate::{DetectError, PackageJson};
use detect_indent::detect_indent;
use std::path::{Path, PathBuf};

/// A `package.json` loaded from disk with its raw JSON value and detected indent.
///
/// Preserves the original structure for write-back: unknown fields, key ordering,
/// and indentation are all retained. Both NCE and NPD use this for mutations.
#[derive(Debug, Clone)]
pub struct PackageJsonFile {
    /// Typed representation (engines, deps, etc.).
    pub parsed: PackageJson,
    /// Raw JSON value — mutate this for write-back to preserve unknown fields.
    pub raw: serde_json::Value,
    /// Detected indentation string from the original file (e.g. `"  "` or `"\t"`).
    pub indent: String,
    /// Path to the file on disk.
    pub path: PathBuf,
}

impl PackageJsonFile {
    /// Read and parse a `package.json` from the given path.
    ///
    /// # Errors
    ///
    /// Returns [`DetectError`] if the file can't be read or parsed.
    pub fn read(path: &Path) -> Result<Self, DetectError> {
        let content = std::fs::read_to_string(path).map_err(|e| DetectError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let indent = detect_indent(&content).indent().to_string();
        let parsed: PackageJson = serde_json::from_str(&content)?;
        let raw: serde_json::Value = serde_json::from_str(&content)?;

        Ok(Self {
            parsed,
            raw,
            indent,
            path: path.to_path_buf(),
        })
    }

    /// Write the raw JSON value back to disk, preserving the original indentation
    /// and ensuring a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`DetectError::Io`] if the file can't be written.
    pub fn write(&self) -> Result<(), DetectError> {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(self.indent.as_bytes());
        let mut buf = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
        serde::Serialize::serialize(&self.raw, &mut serializer).map_err(|e| DetectError::Io {
            path: self.path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        buf.push(b'\n');

        std::fs::write(&self.path, &buf).map_err(|e| DetectError::Io {
            path: self.path.clone(),
            source: e,
        })
    }
}
