#![allow(clippy::tests_outside_test_module)]

use riri_semver_range::{ParsedRange, is_subset_of, restrictive_range};

fn parse(s: &str) -> ParsedRange {
    ParsedRange::parse(s).unwrap_or_else(|e| panic!("parse({s:?}): {e}"))
}

fn check_restrictive(r1: &str, r2: &str, expected: &str) {
    let result = restrictive_range(&parse(r1), &parse(r2));
    assert_eq!(
        result.humanize(),
        expected,
        "restrictive_range({r1:?}, {r2:?})"
    );
}

fn check_humanize(input: &str, expected: &str) {
    let r = parse(input);
    assert_eq!(r.humanize(), expected, "humanize({input:?})");
}

// ===== is_subset_of =====

#[test]
fn subset_same_range() {
    let r = parse("^1.2.3");
    assert!(is_subset_of(&r, &r));
}

#[test]
fn subset_caret_within_gte() {
    let r1 = parse("^16.14.0");
    let r2 = parse(">=16.0.0 <18.0.0");
    assert!(is_subset_of(&r1, &r2));
    assert!(!is_subset_of(&r2, &r1));
}

#[test]
fn subset_wildcard_contains_all() {
    let r1 = parse("^1.0.0");
    let r2 = parse("*");
    assert!(is_subset_of(&r1, &r2));
    assert!(!is_subset_of(&r2, &r1));
}

#[test]
fn subset_disjoint() {
    let r1 = parse("^12.0.0");
    let r2 = parse("^16.0.0");
    assert!(!is_subset_of(&r1, &r2));
    assert!(!is_subset_of(&r2, &r1));
}

// ===== restrictive_range =====

#[test]
fn restrictive_subset_returns_smaller() {
    check_restrictive("^16.14.0", ">=16.0.0 <18.0.0", "^16.14.0");
}

#[test]
fn restrictive_no_intersection_returns_base() {
    check_restrictive("^12.0.0", "^16.0.0", "^12.0.0");
}

#[test]
fn restrictive_align_majors() {
    let result = restrictive_range(
        &parse("^14.17.0 || ^16.10.0 || >=17.0.0"),
        &parse("^16.13.0 || ^18.10.0"),
    );
    let h = result.humanize();
    assert!(h.contains("^16.13.0"), "expected ^16.13.0 in {h:?}");
    assert!(h.contains("^18.10.0"), "expected ^18.10.0 in {h:?}");
}

#[test]
fn restrictive_identical() {
    check_restrictive("^16.0.0", "^16.0.0", "^16.0.0");
}

#[test]
fn restrictive_both_wildcards() {
    check_restrictive("*", "*", "*");
}

#[test]
fn restrictive_one_wildcard() {
    check_restrictive("^1.2.3", "*", "^1.2.3");
    check_restrictive("*", "^1.2.3", "^1.2.3");
}

#[test]
fn restrictive_r2_subset_of_r1() {
    // When r2 is more restrictive, return r2
    check_restrictive(">=16.0.0", "^16.14.0", "^16.14.0");
}

#[test]
fn restrictive_overlapping_ranges() {
    // ^1.0.0 = >=1.0.0 <2.0.0, >=1.5.0 = >=1.5.0
    // intersection = >=1.5.0 <2.0.0 = ^1.5.0
    check_restrictive("^1.0.0", ">=1.5.0", "^1.5.0");
}

#[test]
fn restrictive_multiple_or_parts_both_sides() {
    // Both have ||, need to compute tightest intersection
    check_restrictive(
        "^14.0.0 || ^16.0.0 || ^18.0.0",
        "^16.0.0 || ^18.0.0 || ^20.0.0",
        "^16.0.0 || ^18.0.0",
    );
}

#[test]
fn restrictive_same_major_different_min() {
    // Both target major 16 but different minimums
    check_restrictive("^16.10.0", "^16.13.0", "^16.13.0");
}

#[test]
fn restrictive_commutative_for_subsets() {
    // When one is subset of other, order matters: r1 is base
    let r1 = "^16.14.0";
    let r2 = ">=16.0.0 <18.0.0";
    // r1 ⊂ r2 → returns r1
    check_restrictive(r1, r2, "^16.14.0");
    // r2 is base now, r1 ⊂ r2 → returns r1 (the subset)
    check_restrictive(r2, r1, "^16.14.0");
}

// ===== restrictive_range: complex cases =====

#[test]
fn restrictive_disjoint_or_with_bounded_range() {
    // The original proptest bug: r2 has a part outside r1's bounds
    check_restrictive(">=5.0.0 <9.0.0", "^5.0.0 || ^11.0.0", "^5.0.0");
}

#[test]
fn restrictive_open_ended_with_or() {
    // Open-ended r1 should keep matching r2 parts
    check_restrictive(
        ">=14.0.0",
        "^14.17.0 || ^16.10.0 || >=18.0.0",
        "^14.17.0 || ^16.10.0 || >=18.0.0",
    );
}

#[test]
fn restrictive_cross_major_bounded() {
    // r1 spans multiple majors, r2 has specific carets
    check_restrictive(
        ">=14.0.0 <18.0.0",
        "^14.17.0 || ^16.10.0 || >=18.0.0",
        "^14.17.0 || ^16.10.0",
    );
}

#[test]
fn restrictive_both_open_ended() {
    // Two open-ended ranges — result is the higher min
    check_restrictive(">=14.0.0", ">=16.0.0", ">=16.0.0");
}

#[test]
fn restrictive_or_ranges_partial_overlap() {
    // Only some parts overlap
    check_restrictive(
        "^12.0.0 || ^14.0.0 || ^16.0.0",
        "^14.0.0 || ^16.0.0 || ^18.0.0",
        "^14.0.0 || ^16.0.0",
    );
}

#[test]
fn restrictive_tighter_min_within_same_caret() {
    // Both are carets on same major but different min patches
    check_restrictive("^16.10.0", "^16.13.0", "^16.13.0");
}

#[test]
fn restrictive_bounded_vs_open_ended_same_major() {
    // Bounded at major 16 vs open-ended from major 16
    check_restrictive(">=16.0.0 <17.0.0", ">=16.10.0", "^16.10.0");
}

#[test]
fn restrictive_many_or_parts() {
    // Many OR parts, only a few overlap
    check_restrictive(
        "^10.0.0 || ^12.0.0 || ^14.0.0 || ^16.0.0 || ^18.0.0",
        "^14.0.0 || ^18.0.0 || ^20.0.0",
        "^14.0.0 || ^18.0.0",
    );
}

#[test]
fn restrictive_exact_version_in_range() {
    // Exact version within a caret range
    check_restrictive("^16.0.0", "16.5.0", "16.5.0");
}

#[test]
fn restrictive_zero_major_ranges() {
    // Zero-major caret has special semantics
    check_restrictive("^0.1.0", "^0.1.5", "^0.1.5");
}

// ===== humanize =====

#[test]
fn humanize_caret() {
    check_humanize("^1.2.3", "^1.2.3");
}

#[test]
fn humanize_gte() {
    check_humanize(">=17.0.0", ">=17.0.0");
}

#[test]
fn humanize_wildcard() {
    check_humanize("*", "*");
    check_humanize("", "*");
    check_humanize("x", "*");
}

#[test]
fn humanize_or_range() {
    check_humanize("^14.17.0 || ^16.10.0", "^14.17.0 || ^16.10.0");
}

#[test]
fn humanize_cross_major_split() {
    // >=16.10.0 <18.0.0 spans major 16 & 17, should split
    check_humanize(">=16.10.0 <18.0.0", "^16.10.0 || ^17.0.0");
}

#[test]
fn humanize_exact_version() {
    check_humanize("1.2.3", "1.2.3");
}

#[test]
fn humanize_gt() {
    check_humanize(">1.0.0", ">1.0.0");
}

#[test]
fn humanize_bounded_range() {
    // >=1.0.0 <1.5.0 is not a caret (doesn't go up to next major)
    check_humanize(">=1.0.0 <1.5.0", ">=1.0.0 <1.5.0");
}

#[test]
fn humanize_lte_cross_major() {
    // <=2.0.0 spans majors 0, 1, and includes exactly 2.0.0
    // ^0.0.0 in node-semver means >=0.0.0 <0.0.1, so we can't use caret for major 0.
    check_humanize("<=2.0.0", ">=0.0.0 <1.0.0 || ^1.0.0 || 2.0.0");
}

#[test]
fn humanize_zero_caret() {
    check_humanize("^0.1.2", "^0.1.2");
}

#[test]
fn humanize_multi_major_or() {
    check_humanize(
        "^14.17.0 || ^16.10.0 || >=17.0.0",
        "^14.17.0 || ^16.10.0 || >=17.0.0",
    );
}

#[test]
fn humanize_cross_three_majors() {
    // >=14.0.0 <17.0.0 spans 14, 15, 16
    check_humanize(">=14.0.0 <17.0.0", "^14.0.0 || ^15.0.0 || ^16.0.0");
}

#[test]
fn humanize_round_trip() {
    // Humanized output should re-parse to an equivalent range
    let inputs = [
        "^1.2.3",
        ">=1.0.0",
        "*",
        "^14.17.0 || ^16.10.0",
        "1.2.3",
        ">1.0.0",
    ];
    for input in &inputs {
        let r = parse(input);
        let h = r.humanize();
        let r2 = parse(&h);
        // Check that they agree on a set of versions
        for v_str in &[
            "0.0.0", "1.0.0", "1.2.3", "2.0.0", "14.17.0", "16.10.0", "20.0.0",
        ] {
            let v = semver::Version::parse(v_str).expect("parse version");
            assert_eq!(
                r.satisfies(&v),
                r2.satisfies(&v),
                "round-trip mismatch for {input:?} → {h:?} at {v_str}"
            );
        }
    }
}

// ===== humanize_with precision =====

fn check_humanize_with(
    input: &str,
    precision: riri_semver_range::VersionPrecision,
    expected: &str,
) {
    let r = parse(input);
    assert_eq!(
        r.humanize_with(precision),
        expected,
        "humanize_with({input:?}, {precision:?})"
    );
}

#[test]
fn humanize_with_full_is_default() {
    use riri_semver_range::VersionPrecision;
    // Full precision should match humanize()
    let inputs = ["^1.2.3", ">=17.0.0", ">=24.0.0", "*"];
    for input in &inputs {
        let r = parse(input);
        assert_eq!(r.humanize(), r.humanize_with(VersionPrecision::Full));
    }
}

#[test]
fn humanize_with_major_trims_trailing_zeros() {
    use riri_semver_range::VersionPrecision;
    check_humanize_with(">=24.0.0", VersionPrecision::Major, ">=24");
    check_humanize_with("^1.0.0", VersionPrecision::Major, "^1");
    check_humanize_with(">=16.0.0", VersionPrecision::Major, ">=16");
}

#[test]
fn humanize_with_major_preserves_nonzero() {
    use riri_semver_range::VersionPrecision;
    // Non-zero components are never trimmed
    check_humanize_with(">=16.10.0", VersionPrecision::Major, ">=16.10");
    check_humanize_with("^1.2.3", VersionPrecision::Major, "^1.2.3");
    check_humanize_with(">=1.0.5", VersionPrecision::Major, ">=1.0.5");
}

#[test]
fn humanize_with_major_minor_trims_patch_zero() {
    use riri_semver_range::VersionPrecision;
    check_humanize_with(">=24.0.0", VersionPrecision::MajorMinor, ">=24.0");
    check_humanize_with(">=16.10.0", VersionPrecision::MajorMinor, ">=16.10");
    check_humanize_with("^1.0.0", VersionPrecision::MajorMinor, "^1.0");
}

#[test]
fn humanize_with_major_minor_preserves_nonzero_patch() {
    use riri_semver_range::VersionPrecision;
    check_humanize_with("^1.2.3", VersionPrecision::MajorMinor, "^1.2.3");
    check_humanize_with(">=1.0.5", VersionPrecision::MajorMinor, ">=1.0.5");
}

#[test]
fn humanize_with_bounded_range() {
    use riri_semver_range::VersionPrecision;
    // Both bounds should be formatted with the same precision
    check_humanize_with(">=1.0.0 <2.0.0", VersionPrecision::Major, "^1");
    check_humanize_with(">=1.0.0 <1.5.0", VersionPrecision::Major, ">=1 <1.5");
}

#[test]
fn humanize_with_wildcard_unchanged() {
    use riri_semver_range::VersionPrecision;
    check_humanize_with("*", VersionPrecision::Major, "*");
    check_humanize_with("*", VersionPrecision::MajorMinor, "*");
}
