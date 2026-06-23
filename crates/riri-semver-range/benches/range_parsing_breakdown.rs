use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const RANGES: &[(&str, &str)] = &[
    ("or3_caret", "^14.17.0 || ^16.10.0 || >=17.0.0"),
    ("single_gte", ">=16.0.0"),
    ("or2_caret", "^18.0.0 || ^20.0.0"),
    ("multi_cmp", ">=14.0.0 <18.0.0"),
    ("wildcard", "*"),
    ("tilde", "~1.2.3"),
    ("xrange", "1.2.x"),
    ("or_multi", ">=1.0.0 <2.0.0 || >=3.0.0"),
    ("caret_full", "^1.2.3"),
];

fn bench(c: &mut Criterion) {
    for (name, range) in RANGES {
        c.bench_function(name, |b| {
            b.iter(|| black_box(riri_semver_range::ParsedRange::parse(black_box(range))));
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
