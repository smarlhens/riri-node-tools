#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! Heap profiling with dhat.
//! Run with: `cargo test -p riri-nce --test memory_profile -- --ignored --nocapture --test-threads=1`

use riri_common::LockfileEngines;
use riri_nce::{CheckEnginesInput, check_engines};
use riri_npm::NpmPackageLock;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
#[ignore = "heap profiling — run manually with --nocapture"]
fn profile_check_engines_500_deps() {
    let _profiler = dhat::Profiler::new_heap();

    let lock_content =
        std::fs::read_to_string("../../fixtures/npm-v3-500-deps/package-lock.json").unwrap();
    let pkg_content =
        std::fs::read_to_string("../../fixtures/npm-v3-500-deps/package.json").unwrap();

    let lock = NpmPackageLock::parse(&lock_content).unwrap();
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content).unwrap();

    let entries: Vec<_> = lock.engines_iter().collect();
    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: pkg.engines.as_ref(),
        filter_engines: vec![],
        precision: riri_semver_range::VersionPrecision::Full,
    };

    let _output = check_engines(&input);

    // dhat automatically prints heap profile summary on drop
}

#[test]
#[ignore = "heap profiling — run manually with --nocapture"]
fn profile_parse_lockfile_500_deps() {
    let _profiler = dhat::Profiler::new_heap();

    let content =
        std::fs::read_to_string("../../fixtures/npm-v3-500-deps/package-lock.json").unwrap();
    let _lock = NpmPackageLock::parse(&content).unwrap();
}
