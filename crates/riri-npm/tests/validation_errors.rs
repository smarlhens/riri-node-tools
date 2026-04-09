#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]

use riri_npm::{NpmPackageLock, NpmParseError};

#[test]
fn missing_lockfile_version() {
    let content = r#"{"name": "fake", "dependencies": {}}"#;
    let err = NpmPackageLock::parse(content).unwrap_err();
    assert!(
        matches!(err, NpmParseError::MissingVersion),
        "expected MissingVersion, got: {err}"
    );
}

#[test]
fn unsupported_lockfile_version() {
    let content = r#"{"name": "fake", "lockfileVersion": 99}"#;
    let err = NpmPackageLock::parse(content).unwrap_err();
    assert!(
        matches!(err, NpmParseError::UnsupportedVersion(99)),
        "expected UnsupportedVersion(99), got: {err}"
    );
}

#[test]
fn invalid_json() {
    let err = NpmPackageLock::parse("not json").unwrap_err();
    assert!(
        matches!(err, NpmParseError::Json(_)),
        "expected Json error, got: {err}"
    );
}

#[test]
fn empty_packages_v3() {
    let content = r#"{"lockfileVersion": 3, "packages": {}}"#;
    let lock = NpmPackageLock::parse(content).expect("should parse empty packages");
    assert!(lock.entries().is_empty());
}

#[test]
fn v1_no_dependencies_field() {
    let content = r#"{"lockfileVersion": 1}"#;
    let lock = NpmPackageLock::parse(content).expect("should parse v1 without dependencies");
    assert!(lock.entries().is_empty());
}
