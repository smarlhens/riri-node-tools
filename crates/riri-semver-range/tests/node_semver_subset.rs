#![allow(clippy::tests_outside_test_module)]
//! Ported from node-semver test/ranges/subset.js
//! Skipped: includePrerelease cases, prerelease edge cases.

use riri_semver_range::{ParsedRange, is_subset_of};

fn check(sub: &str, dom: &str, expected: bool) {
    let a = ParsedRange::parse(sub).unwrap_or_else(|e| panic!("parse({sub:?}): {e}"));
    let b = ParsedRange::parse(dom).unwrap_or_else(|e| panic!("parse({dom:?}): {e}"));
    assert_eq!(
        is_subset_of(&a, &b),
        expected,
        "{sub:?} ⊂ {dom:?} should be {expected}"
    );
}

#[test]
fn exact_version_subsets() {
    check("1.2.3", "1.2.3", true);
    check("1.2.3", "1.x", true);
    check("1.2.3", ">1.2.0", true);
}

#[test]
fn null_set_is_subset() {
    check("1.2.3 1.2.4", "1.2.3", true);
    check("1.2.3 1.2.4", "1.2.9", true);
    check(">2 <1", "3", true);
}

#[test]
fn or_subsets() {
    check("1.2.3 2.3.4 || 2.3.4", "3", false);
    check("1 || 2 || 3", ">=1.0.0", true);
}

#[test]
fn wildcard_supersets() {
    check("1.2.3", "*", true);
    check("^1.2.3", "*", true);
    check("1 || 2 || 3", "*", true);
    check("*", "*", true);
    check("", "*", true);
    check("*", "", true);
    check("", "", true);
    check("*", ">=0.0.0", true);
}

#[test]
fn caret_or_subsets() {
    check("^2 || ^3 || ^4", ">=1", true);
    check("^2 || ^3 || ^4", ">1", true);
    check("^2 || ^3 || ^4", ">=2", true);
    check("^2 || ^3 || ^4", ">=3", false);
    check(">=1", "^2 || ^3 || ^4", false);
    check(">1", "^2 || ^3 || ^4", false);
    check(">=2", "^2 || ^3 || ^4", false);
    check(">=3", "^2 || ^3 || ^4", false);
    check("^1", "^2 || ^3 || ^4", false);
    check("^2", "^2 || ^3 || ^4", true);
    check("^3", "^2 || ^3 || ^4", true);
    check("^4", "^2 || ^3 || ^4", true);
    check("1.x", "^2 || ^3 || ^4", false);
    check("2.x", "^2 || ^3 || ^4", true);
    check("3.x", "^2 || ^3 || ^4", true);
    check("4.x", "^2 || ^3 || ^4", true);
}

#[test]
fn exact_or_subsets() {
    check(">=1.0.0 <=1.0.0 || 2.0.0", "1.0.0 || 2.0.0", true);
    check("<=1.0.0 >=1.0.0 || 2.0.0", "1.0.0 || 2.0.0", true);
}

#[test]
fn bounded_range_subsets() {
    check(">=1.0.0", "1.0.0", false);
    check(">=1.0.0 <2.0.0", "<2.0.0", true);
    check(">=1.0.0 <2.0.0", ">0.0.0", true);
    check(">=1.0.0 <=1.0.0", "1.0.0", true);
    check(">=1.0.0 <=1.0.0", "2.0.0", false);
    check("<2.0.0", ">=1.0.0 <2.0.0", false);
    check(">=1.0.0", ">=1.0.0 <2.0.0", false);
    check(">=1.0.0 <2.0.0", "<2.0.0", true);
    check(">=1.0.0 <2.0.0", ">=1.0.0", true);
    check(">=1.0.0 <2.0.0", ">1.0.0", false);
    check(">=1.0.0 <=2.0.0", "<2.0.0", false);
    check(">=1.0.0", "<1.0.0", false);
    check("<=1.0.0", ">1.0.0", false);
}

#[test]
fn impossible_range_subsets() {
    check("<=1.0.0 >1.0.0", ">1.0.0", true);
    check("1.0.0 >1.0.0", ">1.0.0", true);
    check("1.0.0 <1.0.0", ">1.0.0", true);
}

#[test]
fn multi_comparator_subsets() {
    check("<1 <2 <3", "<4", true);
    check("<3 <2 <1", "<4", true);
    check(">1 >2 >3", ">0", true);
    check(">3 >2 >1", ">0", true);
    check("<=1 <=2 <=3", "<4", true);
    check("<=3 <=2 <=1", "<4", true);
    check(">=1 >=2 >=3", ">0", true);
    check(">=3 >=2 >=1", ">0", true);
    check(">=3 >=2 >=1", ">=3 >=2 >=1", true);
}

#[test]
fn gt_vs_gte() {
    check(">2.0.0", ">=2.0.0", true);
}
