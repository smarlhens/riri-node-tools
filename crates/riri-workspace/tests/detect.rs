#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]

use riri_workspace::detect;
use std::fs;
use tempfile::TempDir;

fn write_lockfile(dir: &std::path::Path, kind: &str) {
    let name = match kind {
        "npm" => "package-lock.json",
        "pnpm" => "pnpm-lock.yaml",
        "yarn" => "yarn.lock",
        _ => panic!("unknown PM"),
    };
    fs::write(dir.join(name), "").unwrap();
}

#[test]
fn detects_npm_array_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Npm));
    assert_eq!(project.root(), tmp.path());
}

#[test]
fn detects_npm_object_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces":{"packages":["apps/*"]}}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Npm));
}

#[test]
fn detects_yarn_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "yarn");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Yarn));
}

#[test]
fn no_workspaces_field_returns_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"single","version":"1.0.0"}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");
    assert!(detect(tmp.path()).is_none());
}

#[test]
fn missing_package_json_returns_none() {
    let tmp = TempDir::new().unwrap();
    write_lockfile(tmp.path(), "npm");
    assert!(detect(tmp.path()).is_none());
}

#[test]
fn detects_pnpm_workspaces_yaml() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "packages:\n  - 'packages/*'\n",
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Pnpm));
}

#[test]
fn pnpm_yaml_without_packages_is_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "catalog:\n  foo: ^1.0.0\n",
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");
    assert!(detect(tmp.path()).is_none());
}

#[test]
fn pnpm_missing_yaml_is_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");
    assert!(detect(tmp.path()).is_none());
}
