#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! One-time generator for the 500-dep benchmark fixture.
//! Run with: `cargo test -p riri-nce --test generate_benchmark_fixture -- --ignored`

use std::fmt::Write;

/// Engine range patterns to cycle through for varied realistic data.
const RANGE_PATTERNS: &[&str] = &[
    ">=6.9.0",
    ">=12.22.0",
    ">=14.0.0",
    ">=16.0.0",
    ">=18.0.0",
    "^12.13.0 || ^14.15.0 || ^16.10.0 || >=17.0.0",
    ">=16.0.0||^14.17.0",
    "^14.17.0 || ^16.10.0 || >=17.0.0",
    ">=14.0.0",
    "^18.0.0 || ^20.0.0",
    "*",
    ">=0.10.0",
    ">=8.0.0",
    ">=10.13.0",
    "^16.0.0 || ^18.0.0 || ^20.0.0",
];

#[test]
#[ignore = "one-time generator, not a regular test"]
fn generate_500_deps_fixture() {
    let fixture_dir = std::path::Path::new("../../fixtures/npm-v3-500-deps");
    std::fs::create_dir_all(fixture_dir).unwrap();

    let mut packages = String::from("{\n");
    write!(
        packages,
        r#"  "name": "fake-benchmark",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {{
    "": {{
      "name": "fake-benchmark",
      "version": "0.0.0",
      "license": "MIT"
    }}"#
    )
    .unwrap();

    for i in 0..500 {
        let range = RANGE_PATTERNS[i % RANGE_PATTERNS.len()];
        write!(
            packages,
            r#",
    "node_modules/fake-dep-{i}": {{
      "version": "1.0.{i}",
      "engines": {{
        "node": "{range}"
      }}
    }}"#
        )
        .unwrap();
    }

    // Add 20 deps without engines (realistic)
    for i in 500..520 {
        write!(
            packages,
            r#",
    "node_modules/fake-dep-{i}": {{
      "version": "1.0.{i}"
    }}"#
        )
        .unwrap();
    }

    packages.push_str("\n  }\n}\n");

    std::fs::write(fixture_dir.join("package-lock.json"), &packages).unwrap();

    // Verify it parses
    let lock = riri_npm::NpmPackageLock::parse(&packages).unwrap();
    let count = riri_common::LockfileEngines::engines_iter(&lock).count();
    assert_eq!(count, 500, "should have 500 deps with engines");
}
