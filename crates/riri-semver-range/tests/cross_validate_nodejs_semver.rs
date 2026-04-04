#![allow(clippy::tests_outside_test_module)]
//! Cross-validation: compare our `ParsedRange::satisfies()` against the
//! `nodejs-semver` crate for a wide range of inputs.
//! This catches behavioral divergence from node-semver for non-prerelease versions.

use riri_semver_range::ParsedRange;
use semver::Version;

/// Test versions used for cross-validation.
const TEST_VERSIONS: &[&str] = &[
    "0.0.0", "0.0.1", "0.1.0", "0.1.97", "0.2.3", "0.5.4", "0.5.5", "0.6.2", "0.7.0", "0.7.2",
    "0.8.2", "0.9.7", "1.0.0", "1.0.1", "1.0.2", "1.1.0", "1.1.1", "1.2.0", "1.2.3", "1.2.4",
    "1.2.8", "1.3.0", "1.4.2", "1.8.1", "1.9.7", "2.0.0", "2.0.9", "2.1.2", "2.1.3", "2.3.1",
    "2.4.0", "2.4.5", "2.5.0", "3.0.0", "3.2.0", "3.2.1", "3.2.2", "3.3.2", "7.9.9", "14.17.0",
    "16.10.0", "16.13.0", "16.14.0", "17.0.0", "18.10.0", "20.0.0",
];

/// Range strings used for cross-validation (non-loose, non-prerelease).
const TEST_RANGES: &[&str] = &[
    "1.0.0 - 2.0.0",
    "1 - 2",
    "1.0 - 2.0",
    "1.0.0",
    ">=*",
    "",
    "*",
    ">=1.0.0",
    ">1.0.0",
    "<=2.0.0",
    "1",
    "<2.0.0",
    ">=0.1.97",
    "0.1.20 || 1.2.4",
    ">=0.2.3 || <0.0.1",
    "||",
    "2.x.x",
    "1.2.x",
    "1.2.x || 2.x",
    "x",
    "2.*.*",
    "1.2.*",
    "1.2.* || 2.*",
    "2",
    "2.3",
    "~2.4",
    "~>3.2.1",
    "~1",
    "~>1",
    "~1.0",
    "^0",
    "^0.1",
    "^1.0",
    "^1.2",
    "^0.0.1",
    "^0.1.2",
    "^1.2.3",
    "<1",
    ">=1",
    "<1.2",
    ">1",
    ">1.2",
    "~0.0.1",
    "^1.2 ^1",
    "=0.7.x",
    "<=0.7.x",
    ">=0.7.x",
    "<=7.x",
    "~1.2.1 >=1.2.3",
    ">=1.2.1 >=1.2.3",
    ">=1.0.0 <2.0.0",
    "^14.17.0 || ^16.10.0 || >=17.0.0",
    "^16.13.0 || ^18.10.0",
    "1.2 - 3.4.5",
    "1.2.3 - 3.4",
    "1.2 - 3.4",
    "x - 1.0.0",
    "x - 1.x",
    "1.0.0 - x",
    "1.x - x",
];

fn nodejs_satisfies(range: &str, version: &str) -> Option<bool> {
    let r = nodejs_semver::Range::parse(range).ok()?;
    let v = nodejs_semver::Version::parse(version).ok()?;
    Some(r.satisfies(&v))
}

#[test]
fn cross_validate_satisfies() {
    let mut mismatches = Vec::new();

    for range in TEST_RANGES {
        let Ok(our_range) = ParsedRange::parse(range) else {
            continue;
        };

        for version in TEST_VERSIONS {
            let our_result = our_range.satisfies(&Version::parse(version).expect("valid version"));

            if let Some(nodejs_result) = nodejs_satisfies(range, version)
                && our_result != nodejs_result
            {
                mismatches.push(format!(
                    "range={range:?} version={version:?}: ours={our_result} nodejs={nodejs_result}"
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "Cross-validation found {} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
