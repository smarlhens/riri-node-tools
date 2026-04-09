#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Validation tests for `PackageJson` parsing.

use riri_common::PackageJson;

#[test]
fn parse_minimal() {
    let json = r#"{"name": "foo"}"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    assert_eq!(pkg.name, Some("foo".to_string()));
    assert_eq!(pkg.version, None);
    assert_eq!(pkg.engines, None);
    assert_eq!(pkg.dependencies, None);
}

#[test]
fn parse_with_engines() {
    let json = r#"{"name": "foo", "engines": {"node": ">=16.0.0", "npm": ">=8.0.0"}}"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    let engines = pkg.engines.unwrap();
    assert_eq!(engines.get("node").unwrap(), ">=16.0.0");
    assert_eq!(engines.get("npm").unwrap(), ">=8.0.0");
}

#[test]
fn parse_with_all_dependency_fields() {
    let json = r#"{
        "name": "foo",
        "dependencies": {"a": "^1.0.0"},
        "devDependencies": {"b": "^2.0.0"},
        "optionalDependencies": {"c": "^3.0.0"}
    }"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    assert_eq!(pkg.dependencies.unwrap().get("a").unwrap(), "^1.0.0");
    assert_eq!(pkg.dev_dependencies.unwrap().get("b").unwrap(), "^2.0.0");
    assert_eq!(
        pkg.optional_dependencies.unwrap().get("c").unwrap(),
        "^3.0.0"
    );
}

#[test]
fn parse_empty_object() {
    let json = "{}";
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    assert_eq!(pkg.name, None);
    assert_eq!(pkg.version, None);
    assert_eq!(pkg.engines, None);
    assert_eq!(pkg.dependencies, None);
}

#[test]
fn parse_ignores_unknown_fields() {
    let json = r#"{"name": "foo", "private": true, "scripts": {"test": "echo"}}"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    assert_eq!(pkg.name, Some("foo".to_string()));
}

#[test]
fn parse_invalid_json() {
    let result = serde_json::from_str::<PackageJson>("not json");
    assert!(result.is_err());
}

#[test]
fn parse_engines_empty_object() {
    let json = r#"{"engines": {}}"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    assert!(pkg.engines.unwrap().is_empty());
}

#[test]
fn parse_engines_with_unknown_keys() {
    let json = r#"{"engines": {"node": ">=16", "pnpm": ">=7", "vscode": "^1.60.0"}}"#;
    let pkg: PackageJson = serde_json::from_str(json).unwrap();
    let engines = pkg.engines.unwrap();
    assert_eq!(engines.len(), 3);
    assert_eq!(engines.get("vscode").unwrap(), "^1.60.0");
}
