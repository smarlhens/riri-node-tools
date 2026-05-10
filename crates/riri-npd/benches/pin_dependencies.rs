use criterion::{Criterion, criterion_group, criterion_main};
use riri_common::PackageJson;
use riri_npd::pin_dependencies;
use riri_npm::NpmPackageLock;
use std::hint::black_box;

fn load_fixture(name: &str) -> (NpmPackageLock, PackageJson) {
    let base = format!("../../fixtures/{name}");
    let lock_content = std::fs::read_to_string(format!("{base}/package-lock.json"))
        .unwrap_or_else(|e| panic!("failed to read lockfile for {name}: {e}"));
    let pkg_content = std::fs::read_to_string(format!("{base}/package.json"))
        .unwrap_or_else(|e| panic!("failed to read package.json for {name}: {e}"));

    let lock = NpmPackageLock::parse(&lock_content)
        .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    let pkg: PackageJson = serde_json::from_str(&pkg_content)
        .unwrap_or_else(|e| panic!("failed to parse {name} package.json: {e}"));

    (lock, pkg)
}

fn bench_pin_small(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npd-npm-v3-unpinned-deps");

    c.bench_function("pin_dependencies: 3 deps", |b| {
        b.iter(|| black_box(pin_dependencies(&pkg, &lock).expect("ok")));
    });
}

fn bench_pin_500(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npd-npm-v3-500-deps");

    c.bench_function("pin_dependencies: 500 deps", |b| {
        b.iter(|| black_box(pin_dependencies(&pkg, &lock).expect("ok")));
    });
}

criterion_group!(benches, bench_pin_small, bench_pin_500);
criterion_main!(benches);
