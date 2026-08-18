#![allow(clippy::tests_outside_test_module)]

use riri_npd::{PackageJson, PackageManager, parse_lockfile, pin_dependencies};

const PKG: &str = r#"{"name": "app", "dependencies": {"typescript": "^5.4.0"}}"#;

const NPM_LOCK: &str = r#"{
    "lockfileVersion": 3,
    "packages": {
        "": {"name": "app"},
        "node_modules/typescript": {"version": "5.4.5"}
    }
}"#;

#[test]
fn pinning_needs_no_crate_beyond_riri_npd() {
    let lockfile = parse_lockfile(PackageManager::Npm, NPM_LOCK).expect("lockfile parses");
    let package_json: PackageJson = serde_json::from_str(PKG).expect("package.json parses");

    let pins = pin_dependencies(&package_json, lockfile.versions()).expect("pins");

    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].name, "typescript");
    assert_eq!(pins[0].pinned_version, "5.4.5");
}
