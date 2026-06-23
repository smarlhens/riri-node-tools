use criterion::{Criterion, criterion_group, criterion_main};
use riri_semver_range::{ParsedRange, VersionPrecision};
use std::hint::black_box;

const RANGES: &[&str] = &[
    "^1.2.3",
    ">=16.0.0",
    ">=14.0.0 <18.0.0",
    "^14.17.0 || ^16.10.0 || >=17.0.0",
    "1.2.x",
    "*",
    "~1.2.3",
    ">=1.0.0 <2.0.0 || >=3.0.0",
    "1.0.0",
];

fn bench_humanize(c: &mut Criterion) {
    let parsed: Vec<_> = RANGES
        .iter()
        .map(|r| ParsedRange::parse(r).expect("parse range"))
        .collect();

    c.bench_function("riri: humanize (full)", |b| {
        b.iter(|| {
            for r in &parsed {
                black_box(r.humanize());
            }
        });
    });

    c.bench_function("riri: humanize (major)", |b| {
        b.iter(|| {
            for r in &parsed {
                black_box(r.humanize_with(VersionPrecision::Major));
            }
        });
    });
}

criterion_group!(benches, bench_humanize);
criterion_main!(benches);
