#![allow(clippy::tests_outside_test_module)]
//! Ported from node-semver test/fixtures/range-intersection.js
//! Tests whether two ranges intersect (have any overlapping versions).

use riri_semver_range::ParsedRange;

fn check_intersects(r1: &str, r2: &str, expected: bool) {
    let a = ParsedRange::parse(r1).unwrap_or_else(|e| panic!("parse({r1:?}): {e}"));
    let b = ParsedRange::parse(r2).unwrap_or_else(|e| panic!("parse({r2:?}): {e}"));
    assert_eq!(
        riri_semver_range::intersects(&a, &b),
        expected,
        "intersects({r1:?}, {r2:?}) should be {expected}"
    );
}

#[test]
fn self_intersection() {
    check_intersects("1.3.0 || <1.0.0 >2.0.0", "1.3.0 || <1.0.0 >2.0.0", true);
}

#[test]
fn impossible_ranges() {
    check_intersects("<1.0.0 >2.0.0", ">0.0.0", false);
    check_intersects(">0.0.0", "<1.0.0 >2.0.0", false);
    check_intersects("<1.0.0 >2.0.0", ">1.4.0 <1.6.0", false);
    check_intersects("<1.0.0 >2.0.0", ">1.4.0 <1.6.0 || 2.0.0", false);
    check_intersects(">1.0.0 <1.0.0", "<=0.0.0", false);
}

#[test]
fn boundary_intersections() {
    check_intersects(">1.0.0 <=2.0.0", "2.0.0", true);
    check_intersects("<1.0.0 >=2.0.0", "2.1.0", false);
    check_intersects("<1.0.0 >=2.0.0", ">1.4.0 <1.6.0 || 2.0.0", false);
    check_intersects(">=1.0.0", "<=1.0.0", true);
}

#[test]
fn x_range_no_intersection() {
    check_intersects("1.5.x", "<1.5.0 || >=1.6.0", false);
    check_intersects("<1.5.0 || >=1.6.0", "1.5.x", false);
}

#[test]
fn complex_disjoint() {
    check_intersects(
        "<1.6.16 || >=1.7.0 <1.7.11 || >=1.8.0 <1.8.2",
        ">=1.6.16 <1.7.0 || >=1.7.11 <1.8.0 || >=1.8.2",
        false,
    );
    check_intersects(
        "<=1.6.16 || >=1.7.0 <1.7.11 || >=1.8.0 <1.8.2",
        ">=1.6.16 <1.7.0 || >=1.7.11 <1.8.0 || >=1.8.2",
        true,
    );
}

#[test]
fn wildcard_intersections() {
    check_intersects("*", "0.0.1", true);
    check_intersects("*", ">=1.0.0", true);
    check_intersects("*", ">1.0.0", true);
    check_intersects("*", "~1.0.0", true);
    check_intersects("*", "<1.6.0", true);
    check_intersects("*", "<=1.6.0", true);
    check_intersects("*", "*", true);
    check_intersects("x", "0.0.1", true);
    check_intersects("x", ">=1.0.0", true);
    check_intersects("x", ">1.0.0", true);
    check_intersects("x", "~1.0.0", true);
    check_intersects("x", "<1.6.0", true);
    check_intersects("x", "<=1.6.0", true);
    check_intersects("x", "", true);
}

#[test]
fn major_x_ranges() {
    check_intersects("1.*", "0.0.1", false);
    check_intersects("1.*", "2.0.0", false);
    check_intersects("1.*", "1.0.0", true);
    check_intersects("1.*", "<2.0.0", true);
    check_intersects("1.*", ">1.0.0", true);
    check_intersects("1.*", "<=1.0.0", true);
    check_intersects("1.*", "^1.0.0", true);
    check_intersects("1.x", "0.0.1", false);
    check_intersects("1.x", "2.0.0", false);
    check_intersects("1.x", "1.0.0", true);
    check_intersects("1.x", "<2.0.0", true);
    check_intersects("1.x", ">1.0.0", true);
    check_intersects("1.x", "<=1.0.0", true);
    check_intersects("1.x", "^1.0.0", true);
}

#[test]
fn minor_x_ranges() {
    check_intersects("1.0.*", "0.0.1", false);
    check_intersects("1.0.*", "<0.0.1", false);
    check_intersects("1.0.*", ">0.0.1", true);
    check_intersects("1.0.x", "0.0.1", false);
    check_intersects("1.0.x", "<0.0.1", false);
    check_intersects("1.0.x", ">0.0.1", true);
}

#[test]
fn or_with_wildcard() {
    check_intersects("*", "1.3.0 || <1.0.0 >2.0.0", true);
    check_intersects("1.3.0 || <1.0.0 >2.0.0", "*", true);
    check_intersects("1.*", "1.3.0 || <1.0.0 >2.0.0", true);
    check_intersects("x", "1.3.0 || <1.0.0 >2.0.0", true);
    check_intersects("1.3.0 || <1.0.0 >2.0.0", "x", true);
    check_intersects("1.x", "1.3.0 || <1.0.0 >2.0.0", true);
}
