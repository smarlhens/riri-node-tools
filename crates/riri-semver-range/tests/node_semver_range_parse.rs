#![allow(clippy::tests_outside_test_module)]
//! Ported from node-semver test/fixtures/range-parse.js
//! Tests that valid range strings parse without error and invalid ones produce errors.
//! Also validates structural properties of parsed ranges.
//!
//! Skipped: loose mode, `includePrerelease`, prerelease bounds, `MAX_SAFE_INTEGER`,
//!          >X / <X impossible-range edge cases.

use riri_semver_range::ParsedRange;
use semver::Version;

fn parses_ok(range: &str) {
    ParsedRange::parse(range).unwrap_or_else(|e| panic!("should parse {range:?}: {e}"));
}

fn check_bounds(range: &str, min: (u64, u64, u64), max: Option<(u64, u64, u64)>) {
    let r = ParsedRange::parse(range).unwrap_or_else(|e| panic!("parse({range:?}): {e}"));
    assert_eq!(r.parts.len(), 1, "expected 1 part for {range:?}");
    assert_eq!(
        r.parts[0].min,
        Version::new(min.0, min.1, min.2),
        "min mismatch for {range:?}"
    );
    let expected_max = max.map(|(a, b, c)| Version::new(a, b, c));
    assert_eq!(r.parts[0].max, expected_max, "max mismatch for {range:?}");
}

fn check_satisfies(range: &str, version: &str, expected: bool) {
    let r = ParsedRange::parse(range).unwrap_or_else(|e| panic!("parse({range:?}): {e}"));
    let v = Version::parse(version).unwrap_or_else(|e| panic!("version({version:?}): {e}"));
    assert_eq!(
        r.satisfies(&v),
        expected,
        "{version:?} satisfies({range:?}) should be {expected}"
    );
}

// --- Hyphen ranges ---

#[test]
fn parse_hyphen_full() {
    // '1.0.0 - 2.0.0' → >=1.0.0 <=2.0.0
    let r = ParsedRange::parse("1.0.0 - 2.0.0").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
    assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lte));
}

#[test]
fn parse_hyphen_partial_major() {
    // '1 - 2' → >=1.0.0 <3.0.0
    let r = ParsedRange::parse("1 - 2").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
    assert_eq!(r.parts[0].max, Some(Version::new(3, 0, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lt));
}

#[test]
fn parse_hyphen_partial_minor() {
    // '1.0 - 2.0' → >=1.0.0 <2.1.0
    let r = ParsedRange::parse("1.0 - 2.0").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
    assert_eq!(r.parts[0].max, Some(Version::new(2, 1, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lt));
}

#[test]
fn parse_hyphen_mixed() {
    // '1.2 - 3.4.5' → >=1.2.0 <=3.4.5
    let r = ParsedRange::parse("1.2 - 3.4.5").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 2, 0));
    assert_eq!(r.parts[0].max, Some(Version::new(3, 4, 5)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lte));
}

#[test]
fn parse_hyphen_minor_to_minor() {
    // '1.2 - 3.4' → >=1.2.0 <3.5.0
    let r = ParsedRange::parse("1.2 - 3.4").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 2, 0));
    assert_eq!(r.parts[0].max, Some(Version::new(3, 5, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lt));
}

#[test]
fn parse_hyphen_full_to_minor() {
    // '1.2.3 - 3.4' → >=1.2.3 <3.5.0
    let r = ParsedRange::parse("1.2.3 - 3.4").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
    assert_eq!(r.parts[0].max, Some(Version::new(3, 5, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lt));
}

// --- Wildcards ---

#[test]
fn parse_wildcards() {
    for input in &[">=*", "", "*", "x", "||"] {
        let r = ParsedRange::parse(input).unwrap_or_else(|e| panic!("parse({input:?}): {e}"));
        assert!(
            r.satisfies(&Version::new(0, 0, 0)),
            "{input:?} should satisfy 0.0.0"
        );
        assert!(
            r.satisfies(&Version::new(99, 99, 99)),
            "{input:?} should satisfy 99.99.99"
        );
    }
}

// --- Operators with exact versions ---

#[test]
fn parse_exact_version() {
    check_bounds("1.0.0", (1, 0, 0), Some((1, 0, 1)));
}

#[test]
fn parse_gte() {
    check_bounds(">=1.0.0", (1, 0, 0), None);
}

#[test]
fn parse_gt() {
    let r = ParsedRange::parse(">1.0.0").expect("parse");
    assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
    assert_eq!(r.parts[0].min_op, riri_semver_range::Op::Gt);
    assert!(r.parts[0].max.is_none());
}

#[test]
fn parse_lte() {
    let r = ParsedRange::parse("<=2.0.0").expect("parse");
    assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
    assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lte));
}

#[test]
fn parse_lt() {
    check_bounds("<2.0.0", (0, 0, 0), Some((2, 0, 0)));
}

// --- Partial versions ---

#[test]
fn parse_major_only() {
    // '1' → >=1.0.0 <2.0.0
    check_bounds("1", (1, 0, 0), Some((2, 0, 0)));
}

#[test]
fn parse_major_minor() {
    // '2.3' → >=2.3.0 <2.4.0
    check_bounds("2.3", (2, 3, 0), Some((2, 4, 0)));
}

// --- Whitespace normalization ---

#[test]
fn parse_gte_with_spaces() {
    for input in &[">= 1.0.0", ">=  1.0.0", ">=   1.0.0"] {
        check_bounds(input, (1, 0, 0), None);
    }
}

#[test]
fn parse_gt_with_spaces() {
    for input in &["> 1.0.0", ">  1.0.0"] {
        let r = ParsedRange::parse(input).unwrap_or_else(|e| panic!("parse({input:?}): {e}"));
        assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
        assert_eq!(r.parts[0].min_op, riri_semver_range::Op::Gt);
    }
}

#[test]
fn parse_lte_with_spaces() {
    for input in &["<=   2.0.0", "<= 2.0.0", "<=  2.0.0"] {
        let r = ParsedRange::parse(input).unwrap_or_else(|e| panic!("parse({input:?}): {e}"));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
        assert_eq!(r.parts[0].max_op, Some(riri_semver_range::Op::Lte));
    }
}

#[test]
fn parse_lt_with_spaces() {
    check_bounds("<    2.0.0", (0, 0, 0), Some((2, 0, 0)));
}

#[test]
fn parse_lt_with_tab() {
    // '<\t2.0.0' → <2.0.0
    check_bounds("<\t2.0.0", (0, 0, 0), Some((2, 0, 0)));
}

// --- Tilde with spaces ---

#[test]
fn parse_tilde_with_space() {
    // '~ 1.0' → >=1.0.0 <1.1.0
    check_bounds("~ 1.0", (1, 0, 0), Some((1, 1, 0)));
}

#[test]
fn parse_tilde_gt_with_space() {
    // '~> 1' → >=1.0.0 <2.0.0
    check_bounds("~> 1", (1, 0, 0), Some((2, 0, 0)));
}

// --- Caret with spaces and partial versions ---

#[test]
fn parse_caret_with_space() {
    // '^ 1' → >=1.0.0 <2.0.0
    check_bounds("^ 1", (1, 0, 0), Some((2, 0, 0)));
}

#[test]
fn parse_caret_zero() {
    // '^0' → >=0.0.0 <1.0.0
    check_bounds("^0", (0, 0, 0), Some((1, 0, 0)));
}

#[test]
fn parse_caret_zero_minor_partial() {
    // '^0.1' → >=0.1.0 <0.2.0
    check_bounds("^0.1", (0, 1, 0), Some((0, 2, 0)));
}

#[test]
fn parse_caret_major_partial() {
    // '^1.0' → >=1.0.0 <2.0.0
    check_bounds("^1.0", (1, 0, 0), Some((2, 0, 0)));
}

#[test]
fn parse_caret_major_minor_partial() {
    // '^1.2' → >=1.2.0 <2.0.0
    check_bounds("^1.2", (1, 2, 0), Some((2, 0, 0)));
}

// --- X-ranges ---

#[test]
fn parse_x_ranges() {
    for (input, min, max) in &[
        ("2.x.x", (2, 0, 0), (3, 0, 0)),
        ("1.2.x", (1, 2, 0), (1, 3, 0)),
        ("2.*.*", (2, 0, 0), (3, 0, 0)),
        ("1.2.*", (1, 2, 0), (1, 3, 0)),
        ("2", (2, 0, 0), (3, 0, 0)),
        ("2.3", (2, 3, 0), (2, 4, 0)),
    ] {
        check_bounds(input, *min, Some(*max));
    }
}

// --- Tilde variants ---

#[test]
fn parse_tilde_variants() {
    for (input, min, max) in &[
        ("~2.4", (2, 4, 0), (2, 5, 0)),
        ("~>3.2.1", (3, 2, 1), (3, 3, 0)),
        ("~1", (1, 0, 0), (2, 0, 0)),
        ("~>1", (1, 0, 0), (2, 0, 0)),
        ("~1.0", (1, 0, 0), (1, 1, 0)),
    ] {
        check_bounds(input, *min, Some(*max));
    }
}

// --- Caret variants ---

#[test]
fn parse_caret_variants() {
    for (input, min, max) in &[
        ("^0.0.1", (0, 0, 1), (0, 0, 2)),
        ("^0.1.2", (0, 1, 2), (0, 2, 0)),
        ("^1.2.3", (1, 2, 3), (2, 0, 0)),
    ] {
        check_bounds(input, *min, Some(*max));
    }
}

// --- Partial versions with operators ---

#[test]
fn parse_lt_partial() {
    for (input, max) in &[
        ("<1", (1, 0, 0)),
        ("< 1", (1, 0, 0)),
        ("<1.2", (1, 2, 0)),
        ("< 1.2", (1, 2, 0)),
    ] {
        check_bounds(input, (0, 0, 0), Some(*max));
    }
}

#[test]
fn parse_gte_partial() {
    for (input, min) in &[(">=1", (1, 0, 0)), (">= 1", (1, 0, 0))] {
        check_bounds(input, *min, None);
    }
}

// --- Greater-than with partial (rounds up) ---

#[test]
fn parse_gt_partial_round_up() {
    // '>1' → >=2.0.0
    check_bounds(">1", (2, 0, 0), None);
    // '>1.2' → >=1.3.0
    check_bounds(">1.2", (1, 3, 0), None);
}

// --- OR ranges ---

#[test]
fn parse_or_range() {
    let r = ParsedRange::parse("0.1.20 || 1.2.4").expect("parse");
    assert_eq!(r.parts.len(), 2);
    check_satisfies("0.1.20 || 1.2.4", "0.1.20", true);
    check_satisfies("0.1.20 || 1.2.4", "1.2.4", true);
    check_satisfies("0.1.20 || 1.2.4", "1.2.3", false);
}

#[test]
fn parse_or_with_operators() {
    let r = ParsedRange::parse(">=0.2.3 || <0.0.1").expect("parse");
    assert_eq!(r.parts.len(), 2);
    check_satisfies(">=0.2.3 || <0.0.1", "0.0.0", true);
    check_satisfies(">=0.2.3 || <0.0.1", "0.2.3", true);
}

// --- Multi-comparator with space-separated operators ---

#[test]
fn parse_multi_comparator_with_space() {
    // '^ 1.2 ^ 1' → intersect ^1.2 and ^1 → >=1.2.0 <2.0.0
    let r = ParsedRange::parse("^ 1.2 ^ 1").expect("parse");
    assert_eq!(r.parts.len(), 1);
    check_satisfies("^ 1.2 ^ 1", "1.4.2", true);
    check_satisfies("^ 1.2 ^ 1", "1.1.0", false);
}

// --- Build metadata in ranges (stripped) ---

#[test]
fn parse_build_metadata_stripped() {
    for input in &["^1.2.3+build", "1.x.x+build", ">=1.x+build", "~1.x+build"] {
        parses_ok(input);
    }

    check_satisfies("^1.2.3+build", "1.2.3", true);
    check_satisfies("^1.2.3+build", "1.3.0", true);
    check_satisfies("^1.2.3+build", "2.0.0", false);
}

// --- v prefix ---

#[test]
fn parse_v_prefix() {
    parses_ok("v1.0.0");
    check_satisfies("v1.0.0", "1.0.0", true);
}

// --- All valid ranges from upstream should parse ---

#[test]
fn all_valid_ranges_parse() {
    let valid_ranges = [
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
        ">= 1.0.0",
        ">=  1.0.0",
        ">=   1.0.0",
        "> 1.0.0",
        ">  1.0.0",
        "<=   2.0.0",
        "<= 2.0.0",
        "<=  2.0.0",
        "<    2.0.0",
        "<\t2.0.0",
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
        "~> 1",
        "~1.0",
        "~ 1.0",
        "^0",
        "^ 1",
        "^0.1",
        "^1.0",
        "^1.2",
        "^0.0.1",
        "^0.1.2",
        "^1.2.3",
        "<1",
        "< 1",
        ">=1",
        ">= 1",
        "<1.2",
        "< 1.2",
        "^ 1.2 ^ 1",
        "1.2 - 3.4.5",
        "1.2.3 - 3.4",
        "1.2 - 3.4",
        ">1",
        ">1.2",
    ];

    for range in &valid_ranges {
        parses_ok(range);
    }
}

// --- Impossible ranges (from upstream) ---

#[test]
fn parse_gt_wildcard_impossible() {
    // >X / >* means "greater than everything" → impossible, satisfies nothing
    for input in &[">X", ">x", ">*"] {
        let r = ParsedRange::parse(input).unwrap_or_else(|e| panic!("parse({input:?}): {e}"));
        assert!(
            !r.satisfies(&Version::new(0, 0, 0)),
            "{input:?} should not satisfy 0.0.0"
        );
        assert!(
            !r.satisfies(&Version::new(999, 999, 999)),
            "{input:?} should not satisfy 999.999.999"
        );
    }
}

#[test]
fn parse_lt_wildcard_impossible() {
    // <X / <* means "less than nothing" → impossible
    for input in &["<X", "<x", "<*"] {
        let r = ParsedRange::parse(input).unwrap_or_else(|e| panic!("parse({input:?}): {e}"));
        assert!(
            !r.satisfies(&Version::new(0, 0, 0)),
            "{input:?} should not satisfy 0.0.0"
        );
    }
}
