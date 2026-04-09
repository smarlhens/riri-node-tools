use criterion::{Criterion, criterion_group, criterion_main};
use riri_common::LockfileEngines;
use riri_nce::{CheckEnginesInput, check_engines};
use riri_npm::NpmPackageLock;
use std::hint::black_box;

fn load_fixture(name: &str) -> (NpmPackageLock, riri_common::PackageJson) {
    let base = format!("../../fixtures/{name}");
    let lock_content = std::fs::read_to_string(format!("{base}/package-lock.json"))
        .unwrap_or_else(|e| panic!("failed to read lockfile for {name}: {e}"));
    let pkg_content = std::fs::read_to_string(format!("{base}/package.json"))
        .unwrap_or_else(|e| panic!("failed to read package.json for {name}: {e}"));

    let lock = NpmPackageLock::parse(&lock_content)
        .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    let pkg: riri_common::PackageJson = serde_json::from_str(&pkg_content)
        .unwrap_or_else(|e| panic!("failed to parse {name} package.json: {e}"));

    (lock, pkg)
}

fn bench_check_engines_small(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npm-v3-or-ranges-node-only");

    c.bench_function("check_engines: 7 deps (or-ranges)", |b| {
        b.iter(|| {
            let entries: Vec<_> = lock.engines_iter().collect();
            let input = CheckEnginesInput {
                lockfile_entries: entries,
                package_engines: pkg.engines.as_ref(),
                filter_engines: vec![],
                precision: riri_semver_range::VersionPrecision::Full,
            };
            black_box(check_engines(&input))
        });
    });
}

fn bench_check_engines_500(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npm-v3-500-deps");

    c.bench_function("check_engines: 500 deps", |b| {
        b.iter(|| {
            let entries: Vec<_> = lock.engines_iter().collect();
            let input = CheckEnginesInput {
                lockfile_entries: entries,
                package_engines: pkg.engines.as_ref(),
                filter_engines: vec![],
                precision: riri_semver_range::VersionPrecision::Full,
            };
            black_box(check_engines(&input))
        });
    });
}

fn bench_parse_lockfile_500(c: &mut Criterion) {
    let content = std::fs::read_to_string("../../fixtures/npm-v3-500-deps/package-lock.json")
        .expect("failed to read 500-deps lockfile");

    c.bench_function("parse npm lockfile: 500 deps", |b| {
        b.iter(|| {
            black_box(NpmPackageLock::parse(&content).expect("failed to parse 500-deps lockfile"));
        });
    });
}

criterion_group!(
    benches,
    bench_check_engines_small,
    bench_check_engines_500,
    bench_parse_lockfile_500
);
criterion_main!(benches);
