#![allow(clippy::tests_outside_test_module)]

use riri_nce::{
    CheckEnginesInput, EngineConstraintKey, LifecycleConfig, LifecycleData, NaiveDate, PackageJson,
    PackageManager, Policy, VersionPrecision, check_engines, check_engines_with_lifecycle,
    parse_lockfile,
};

const PKG: &str = r#"{"name": "app", "engines": {"node": ">=14.0.0"}}"#;

const NPM_LOCK: &str = r#"{
    "lockfileVersion": 3,
    "packages": {
        "": {"name": "app"},
        "node_modules/typescript": {"version": "5.4.5", "engines": {"node": ">=14.17.0"}}
    }
}"#;

#[test]
fn checking_engines_needs_no_crate_beyond_riri_nce() {
    let lockfile = parse_lockfile(PackageManager::Npm, NPM_LOCK).expect("lockfile parses");
    let engines = lockfile.engines().expect("npm records engines");
    let package_json: PackageJson = serde_json::from_str(PKG).expect("package.json parses");

    let output = check_engines(&CheckEnginesInput {
        lockfile_entries: engines.engines_iter().collect(),
        package_engines: package_json.engines.as_ref(),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    });

    assert_eq!(
        output.computed_engines[&EngineConstraintKey::Node],
        ">=14.17.0"
    );
}

#[test]
fn the_lifecycle_rewrite_needs_no_crate_beyond_riri_nce() {
    let lockfile = parse_lockfile(PackageManager::Npm, NPM_LOCK).expect("lockfile parses");
    let engines = lockfile.engines().expect("npm records engines");
    let package_json: PackageJson = serde_json::from_str(PKG).expect("package.json parses");

    let input = CheckEnginesInput {
        lockfile_entries: engines.engines_iter().collect(),
        package_engines: package_json.engines.as_ref(),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    };
    let config = LifecycleConfig {
        data: LifecycleData::bundled(),
        policy: Policy::Supported,
        today: NaiveDate::from_ymd_opt(2026, 8, 18).expect("valid date"),
        allow_eol: true,
        bump_npm: false,
        npm_precision: VersionPrecision::Full,
    };

    let (output, _lifecycle) =
        check_engines_with_lifecycle(&input, &config).expect("lifecycle rewrite");

    assert!(
        output
            .computed_engines
            .contains_key(&EngineConstraintKey::Node)
    );
}
