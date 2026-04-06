#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Test `check_engines` against pnpm fixtures.
//!
//! Cross-parity: these fixtures produce the same computation
//! results as their npm equivalents.

use riri_common::LockfileEngines;
use riri_nce::{CheckEnginesInput, check_engines};
use riri_pnpm::PnpmLockfile;

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
