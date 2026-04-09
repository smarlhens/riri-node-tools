#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Parse all npm fixtures and snapshot the engine entries.

use riri_common::{Engines, LockfileEngines};
use riri_npm::NpmPackageLock;
use rstest::rstest;
use std::path::PathBuf;

/// Format engines deterministically (sorted keys for objects).
fn fmt_engines(engines: &Engines) -> String {
    match engines {
        Engines::Object(map) => {
            let mut pairs: Vec<_> = map.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            let inner = pairs
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        Engines::Array(arr) => format!("{arr:?}"),
    }
}

#[rstest]
fn parse_npm_fixture(#[files("../../fixtures/npm-*/package-lock.json")] lockfile_path: PathBuf) {
    let fixture_name = lockfile_path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let content = std::fs::read_to_string(&lockfile_path).unwrap();
    let lock = NpmPackageLock::parse(&content)
        .unwrap_or_else(|e| panic!("failed to parse {fixture_name}: {e}"));

    // Collect engine entries sorted by name for deterministic snapshots
    let mut engines: Vec<(&str, &Engines)> = lock.engines_iter().collect();
    engines.sort_by_key(|(name, _)| *name);

    let snapshot = engines
        .iter()
        .map(|(name, eng)| format!("{name}: {}", fmt_engines(eng)))
        .collect::<Vec<_>>()
        .join("\n");

    insta::with_settings!({snapshot_suffix => fixture_name}, {
        insta::assert_snapshot!(snapshot);
    });
}
