#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Parse all pnpm fixtures and snapshot the engine entries.

use riri_common::{Engines, LockfileEngines};
use riri_pnpm::PnpmLockfile;
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
fn parse_pnpm_fixture(#[files("../../fixtures/pnpm-*/pnpm-lock.yaml")] lockfile_path: PathBuf) {
    let fixture_name = lockfile_path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let content = std::fs::read_to_string(&lockfile_path).unwrap();
    let lock = PnpmLockfile::parse(&content)
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

#[test]
fn missing_lockfile_version() {
    let err = PnpmLockfile::parse("packages: {}").unwrap_err();
    assert!(
        matches!(err, riri_pnpm::PnpmParseError::MissingVersion),
        "expected MissingVersion, got: {err}"
    );
}

#[test]
fn unsupported_lockfile_version() {
    let err = PnpmLockfile::parse("lockfileVersion: '99.0'\npackages: {}").unwrap_err();
    assert!(
        matches!(err, riri_pnpm::PnpmParseError::UnsupportedVersion(_)),
        "expected UnsupportedVersion, got: {err}"
    );
}

#[test]
fn invalid_yaml() {
    let err = PnpmLockfile::parse("{{{{invalid yaml").unwrap_err();
    assert!(
        matches!(err, riri_pnpm::PnpmParseError::Yaml(_)),
        "expected Yaml error, got: {err}"
    );
}

#[test]
fn empty_packages() {
    let lock = PnpmLockfile::parse("lockfileVersion: 5.4\npackages: {}").unwrap();
    assert_eq!(lock.engines_iter().count(), 0);
}
