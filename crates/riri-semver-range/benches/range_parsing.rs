use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const RANGES: &[&str] = &[
    "^14.17.0 || ^16.10.0 || >=17.0.0",
    ">=16.0.0",
    "^18.0.0 || ^20.0.0",
    ">=14.0.0 <18.0.0",
    "*",
    "~1.2.3",
    "1.2.x",
    ">=1.0.0 <2.0.0 || >=3.0.0",
];

fn bench_riri_parse(c: &mut Criterion) {
    c.bench_function("riri: parse range", |b| {
        b.iter(|| {
            for range in RANGES {
                let _ = black_box(riri_semver_range::ParsedRange::parse(range));
            }
        });
    });
}

fn bench_nodejs_semver_parse(c: &mut Criterion) {
    c.bench_function("nodejs-semver: parse range", |b| {
        b.iter(|| {
            for range in RANGES {
                let _ = black_box(nodejs_semver::Range::parse(range));
            }
        });
    });
}

criterion_group!(benches, bench_riri_parse, bench_nodejs_semver_parse);
criterion_main!(benches);
