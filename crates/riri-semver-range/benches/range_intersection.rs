use criterion::{Criterion, criterion_group, criterion_main};
use riri_semver_range::{ParsedRange, restrictive_range};
use std::hint::black_box;

/// Pairs of ranges to intersect, representing realistic lockfile scenarios.
const INTERSECTION_PAIRS: &[(&str, &str)] = &[
    (">=14.0.0", ">=16.0.0"),
    ("^14.17.0 || ^16.10.0 || >=17.0.0", ">=12.22.0"),
    (">=6.9.0", "^12.13.0 || ^14.15.0 || ^16.10.0 || >=17.0.0"),
    (">=16.0.0||^14.17.0", ">=12.22.0"),
    ("*", ">=16.0.0"),
    (">=18.0.0", ">=14.0.0 <22.0.0"),
    ("^18.0.0 || ^20.0.0", ">=16.0.0"),
    (">=1.0.0 <2.0.0 || >=3.0.0", ">=1.5.0"),
];

fn bench_restrictive_range(c: &mut Criterion) {
    let parsed_pairs: Vec<(ParsedRange, ParsedRange)> = INTERSECTION_PAIRS
        .iter()
        .map(|(a, b)| {
            (
                ParsedRange::parse(a).expect("valid range"),
                ParsedRange::parse(b).expect("valid range"),
            )
        })
        .collect();

    c.bench_function("riri: restrictive_range (8 pairs)", |b| {
        b.iter(|| {
            for (a, r_b) in &parsed_pairs {
                let _ = black_box(restrictive_range(a, r_b));
            }
        });
    });
}

fn bench_parse_and_intersect(c: &mut Criterion) {
    c.bench_function("riri: parse + restrictive_range (8 pairs)", |b| {
        b.iter(|| {
            for (a, r_b) in INTERSECTION_PAIRS {
                let pa = ParsedRange::parse(a).expect("valid");
                let pb = ParsedRange::parse(r_b).expect("valid");
                let _ = black_box(restrictive_range(&pa, &pb));
            }
        });
    });
}

criterion_group!(benches, bench_restrictive_range, bench_parse_and_intersect);
criterion_main!(benches);
