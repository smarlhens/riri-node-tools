use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const CASES: &[(&str, &str)] = &[
    ("^14.17.0 || ^16.10.0 || >=17.0.0", "16.14.0"),
    (">=16.0.0", "18.0.0"),
    ("^18.0.0 || ^20.0.0", "20.1.0"),
    (">=14.0.0 <18.0.0", "15.0.0"),
    ("*", "1.0.0"),
    ("~1.2.3", "1.2.5"),
    (">=1.0.0 <2.0.0 || >=3.0.0", "3.5.0"),
];

fn bench_riri_satisfies(c: &mut Criterion) {
    let parsed: Vec<_> = CASES
        .iter()
        .map(|(r, v)| {
            (
                riri_semver_range::ParsedRange::parse(r).expect("parse range"),
                semver::Version::parse(v).expect("parse version"),
            )
        })
        .collect();

    c.bench_function("riri: satisfies", |b| {
        b.iter(|| {
            for (range, version) in &parsed {
                black_box(range.satisfies(version));
            }
        });
    });
}

fn bench_nodejs_semver_satisfies(c: &mut Criterion) {
    let parsed: Vec<_> = CASES
        .iter()
        .map(|(r, v)| {
            (
                nodejs_semver::Range::parse(r).expect("parse range"),
                nodejs_semver::Version::parse(v).expect("parse version"),
            )
        })
        .collect();

    c.bench_function("nodejs-semver: satisfies", |b| {
        b.iter(|| {
            for (range, version) in &parsed {
                black_box(range.satisfies(version));
            }
        });
    });
}

criterion_group!(benches, bench_riri_satisfies, bench_nodejs_semver_satisfies);
criterion_main!(benches);
