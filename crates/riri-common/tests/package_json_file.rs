#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Tests for `PackageJsonFile` read/write with indent preservation.

use riri_common::PackageJsonFile;
use std::fs;
use tempfile::TempDir;

#[test]
fn read_detects_2_space_indent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    fs::write(&path, "{\n  \"name\": \"foo\"\n}\n").unwrap();

    let pkg = PackageJsonFile::read(&path).unwrap();
    assert_eq!(pkg.indent, "  ");
    assert_eq!(pkg.parsed.name, Some("foo".to_string()));
}

#[test]
fn read_detects_4_space_indent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    fs::write(&path, "{\n    \"name\": \"bar\"\n}\n").unwrap();

    let pkg = PackageJsonFile::read(&path).unwrap();
    assert_eq!(pkg.indent, "    ");
}

#[test]
fn read_detects_tab_indent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    fs::write(&path, "{\n\t\"name\": \"baz\"\n}\n").unwrap();

    let pkg = PackageJsonFile::read(&path).unwrap();
    assert_eq!(pkg.indent, "\t");
}

#[test]
fn write_preserves_indent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    let original = "{\n    \"name\": \"test\"\n}\n";
    fs::write(&path, original).unwrap();

    let pkg = PackageJsonFile::read(&path).unwrap();
    pkg.write().unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("    \"name\""),
        "should preserve 4-space indent"
    );
    assert!(written.ends_with('\n'), "should have trailing newline");
}

#[test]
fn write_preserves_unknown_fields() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    let original = "{\n  \"name\": \"test\",\n  \"private\": true,\n  \"scripts\": {\n    \"test\": \"echo\"\n  }\n}\n";
    fs::write(&path, original).unwrap();

    let pkg = PackageJsonFile::read(&path).unwrap();
    pkg.write().unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("\"private\": true"),
        "should preserve unknown fields"
    );
    assert!(written.contains("\"scripts\""), "should preserve scripts");
}

#[test]
fn read_nonexistent_file_errors() {
    let result = PackageJsonFile::read(std::path::Path::new("/nonexistent/package.json"));
    assert!(result.is_err());
}

#[test]
fn raw_value_can_be_mutated() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("package.json");
    fs::write(&path, "{\n  \"name\": \"test\"\n}\n").unwrap();

    let mut pkg = PackageJsonFile::read(&path).unwrap();

    // Add an engines field via raw Value
    pkg.raw.as_object_mut().unwrap().insert(
        "engines".to_string(),
        serde_json::json!({"node": ">=18.0.0"}),
    );
    pkg.write().unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("\"engines\""));
    assert!(written.contains(">=18.0.0"));
}
