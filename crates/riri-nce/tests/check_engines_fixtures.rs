#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Test `check_engines` against npm & pnpm fixtures.
//!
//! For each fixture, parse the lockfile and package.json,
//! run `check_engines`, and snapshot the result.

use riri_common::{EngineConstraintKey, LockfileEngines};
use riri_nce::{CheckEnginesInput, check_engines};
use riri_npm::NpmPackageLock;
use riri_pnpm::PnpmLockfile;
use riri_semver_range::VersionPrecision;
use std::collections::HashMap;

fn run_fixture(fixture_dir: &str) -> String {
    let base = format!("../../fixtures/{fixture_dir}");
    let lock_path = format!("{base}/package-lock.json");
    let pkg_path = format!("{base}/package.json");

    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    let pkg_content = std::fs::read_to_string(&pkg_path).unwrap();

    let lock = NpmPackageLock::parse(&lock_content).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();

    let entries: Vec<(&str, &riri_common::Engines)> = lock.engines_iter().collect();

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![],
        precision: VersionPrecision::Full,
    };

    let output = check_engines(&input);

    let mut lines = Vec::new();

    // Show computed engines (sorted by key for determinism)
    let mut computed: Vec<_> = output.computed_engines.iter().collect();
    computed.sort_by_key(|(k, _)| format!("{k}"));
    for (key, range) in &computed {
        lines.push(format!("computed {key}: {range}"));
    }

    // Show changes
    if output.engines_range_to_set.is_empty() {
        lines.push("no changes needed".to_string());
    } else {
        for change in &output.engines_range_to_set {
            lines.push(format!(
                "change {}: {} → {}",
                change.engine, change.range, change.range_to_set
            ));
        }
    }

    lines.join("\n")
}

#[test]
fn npm_or_ranges_node_only() {
    let result = run_fixture("npm-v3-or-ranges-node-only");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_or_ranges_node_npm_yarn() {
    let result = run_fixture("npm-v3-or-ranges-node-npm-yarn");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_no_intersection() {
    let result = run_fixture("npm-v3-no-intersection");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_wildcard_and_missing() {
    let result = run_fixture("npm-v3-wildcard-and-missing");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_engines_as_array() {
    let result = run_fixture("npm-v3-engines-as-array");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_packages_field_v3() {
    let result = run_fixture("npm-v3-packages-field");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_v1_deps_field() {
    let result = run_fixture("npm-v1-deps-field");
    insta::assert_snapshot!(result);
}

#[test]
fn npm_engine_filtering_node_only() {
    let lock_content =
        std::fs::read_to_string("../../fixtures/npm-v3-or-ranges-node-npm-yarn/package-lock.json")
            .unwrap();
    let pkg_content =
        std::fs::read_to_string("../../fixtures/npm-v3-or-ranges-node-npm-yarn/package.json")
            .unwrap();

    let lock = NpmPackageLock::parse(&lock_content).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();
    let entries: Vec<(&str, &riri_common::Engines)> = lock.engines_iter().collect();

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    };

    let output = check_engines(&input);

    // Only node should be computed
    assert_eq!(output.computed_engines.len(), 1);
    assert!(
        output
            .computed_engines
            .contains_key(&EngineConstraintKey::Node)
    );
}

#[test]
fn npm_no_engines_package_json() {
    let lock_content =
        std::fs::read_to_string("../../fixtures/npm-v3-no-npmrc/package-lock.json").unwrap();
    let pkg_content =
        std::fs::read_to_string("../../fixtures/npm-v3-no-npmrc/package.json").unwrap();

    let lock = NpmPackageLock::parse(&lock_content).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();
    let entries: Vec<(&str, &riri_common::Engines)> = lock.engines_iter().collect();

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![],
        precision: VersionPrecision::Full,
    };

    let output = check_engines(&input);

    // Should compute ranges from deps even without root engines
    let node_range = output
        .computed_engines
        .get(&EngineConstraintKey::Node)
        .unwrap();
    assert_ne!(node_range, "*", "should compute a non-wildcard node range");
}

#[test]
fn get_constraint_from_engines_object() {
    let engines = riri_common::Engines::Object(HashMap::from([
        ("node".to_string(), ">=16.0.0".to_string()),
        ("npm".to_string(), ">=8.0.0".to_string()),
    ]));
    assert_eq!(
        riri_nce::get_constraint_from_engines(&engines, EngineConstraintKey::Node),
        Some(">=16.0.0".to_string())
    );
    assert_eq!(
        riri_nce::get_constraint_from_engines(&engines, EngineConstraintKey::Yarn),
        None
    );
}

#[test]
fn get_constraint_from_engines_array() {
    let engines = riri_common::Engines::Array(vec!["node >= 7".to_string()]);
    assert_eq!(
        riri_nce::get_constraint_from_engines(&engines, EngineConstraintKey::Node),
        Some(">= 7".to_string())
    );
    assert_eq!(
        riri_nce::get_constraint_from_engines(&engines, EngineConstraintKey::Npm),
        None
    );
}

// ── normalization detection ─────────────────────────────────────────

#[test]
fn normalization_detects_short_form() {
    // If package.json has ">=24" but the humanized form is ">=24.0.0",
    // check_engines should report a change to normalize the format.
    let pkg_engines = HashMap::from([("node".to_string(), ">=24".to_string())]);
    let input = CheckEnginesInput {
        lockfile_entries: vec![],
        package_engines: Some(&pkg_engines),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    };
    let output = check_engines(&input);
    assert_eq!(output.engines_range_to_set.len(), 1);
    assert_eq!(output.engines_range_to_set[0].range, ">=24");
    assert_eq!(output.engines_range_to_set[0].range_to_set, ">=24.0.0");
}

#[test]
fn normalization_skips_already_full() {
    // If package.json already has the full form, no change needed.
    let pkg_engines = HashMap::from([("node".to_string(), ">=24.0.0".to_string())]);
    let input = CheckEnginesInput {
        lockfile_entries: vec![],
        package_engines: Some(&pkg_engines),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    };
    let output = check_engines(&input);
    assert!(
        output.engines_range_to_set.is_empty(),
        "no changes expected for already-normalized range"
    );
}

#[test]
fn normalization_with_major_precision() {
    // With Major precision, ">=24.0.0" in package.json should be normalized to ">=24".
    let pkg_engines = HashMap::from([("node".to_string(), ">=24.0.0".to_string())]);
    let input = CheckEnginesInput {
        lockfile_entries: vec![],
        package_engines: Some(&pkg_engines),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Major,
    };
    let output = check_engines(&input);
    assert_eq!(output.engines_range_to_set.len(), 1);
    assert_eq!(output.engines_range_to_set[0].range, ">=24.0.0");
    assert_eq!(output.engines_range_to_set[0].range_to_set, ">=24");
}

#[test]
fn normalization_caret_short_form() {
    // "^1" in package.json should be normalized to "^1.0.0" at Full precision.
    let pkg_engines = HashMap::from([("node".to_string(), "^1".to_string())]);
    let input = CheckEnginesInput {
        lockfile_entries: vec![],
        package_engines: Some(&pkg_engines),
        filter_engines: vec![EngineConstraintKey::Node],
        precision: VersionPrecision::Full,
    };
    let output = check_engines(&input);
    assert_eq!(output.engines_range_to_set.len(), 1);
    assert_eq!(output.engines_range_to_set[0].range, "^1");
    assert_eq!(output.engines_range_to_set[0].range_to_set, "^1.0.0");
}

// ── pnpm fixtures ───────────────────────────────────────────────────

fn run_pnpm_fixture(fixture_dir: &str) -> String {
    let base = format!("../../fixtures/{fixture_dir}");
    let lock_path = format!("{base}/pnpm-lock.yaml");
    let pkg_path = format!("{base}/package.json");

    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    let pkg_content = std::fs::read_to_string(&pkg_path).unwrap();

    let lock = PnpmLockfile::parse(&lock_content).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();

    let entries: Vec<(&str, &riri_common::Engines)> = lock.engines_iter().collect();

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![],
        precision: VersionPrecision::Full,
    };

    let output = check_engines(&input);

    let mut lines = Vec::new();

    // Show computed engines (sorted by key for determinism)
    let mut computed: Vec<_> = output.computed_engines.iter().collect();
    computed.sort_by_key(|(k, _)| format!("{k}"));
    for (key, range) in &computed {
        lines.push(format!("computed {key}: {range}"));
    }

    // Show changes
    if output.engines_range_to_set.is_empty() {
        lines.push("no changes needed".to_string());
    } else {
        for change in &output.engines_range_to_set {
            lines.push(format!(
                "change {}: {} → {}",
                change.engine, change.range, change.range_to_set
            ));
        }
    }

    lines.join("\n")
}

#[test]
fn pnpm_or_ranges_node_only() {
    let result = run_pnpm_fixture("pnpm-v9-or-ranges-node-only");
    insta::assert_snapshot!(result);
}

#[test]
fn pnpm_or_ranges_node_npm_yarn() {
    let result = run_pnpm_fixture("pnpm-v9-or-ranges-node-npm-yarn");
    insta::assert_snapshot!(result);
}

// ── yarn fixtures ───────────────────────────────────────────────────

fn run_yarn_fixture(fixture_dir: &str) -> String {
    let fixture_path = std::path::Path::new("../../fixtures").join(fixture_dir);
    let pkg_path = fixture_path.join("package.json");

    let project = riri_yarn::YarnProject::scan(&fixture_path).unwrap();
    let pkg_content = std::fs::read_to_string(&pkg_path).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();

    let entries: Vec<(&str, &riri_common::Engines)> = project.engines_iter().collect();

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![],
        precision: VersionPrecision::Full,
    };

    let output = check_engines(&input);

    let mut lines = Vec::new();

    let mut computed: Vec<_> = output.computed_engines.iter().collect();
    computed.sort_by_key(|(k, _)| format!("{k}"));
    for (key, range) in &computed {
        lines.push(format!("computed {key}: {range}"));
    }

    if output.engines_range_to_set.is_empty() {
        lines.push("no changes needed".to_string());
    } else {
        for change in &output.engines_range_to_set {
            lines.push(format!(
                "change {}: {} → {}",
                change.engine, change.range, change.range_to_set
            ));
        }
    }

    lines.join("\n")
}

#[test]
fn yarn_or_ranges_node_only() {
    let result = run_yarn_fixture("yarn-v1-or-ranges-node-only");
    insta::assert_snapshot!(result);
}

#[test]
fn yarn_or_ranges_node_npm_yarn() {
    let result = run_yarn_fixture("yarn-v4-or-ranges-node-npm-yarn");
    insta::assert_snapshot!(result);
}
