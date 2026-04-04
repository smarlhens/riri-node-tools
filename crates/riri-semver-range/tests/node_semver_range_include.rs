#![allow(clippy::tests_outside_test_module)]
//! Ported from node-semver test/fixtures/range-include.js
//! Tests that version SHOULD satisfy the given range.
//! Skipped: loose mode, includePrerelease, prerelease versions.

use riri_semver_range::ParsedRange;
use semver::Version;

fn check(range: &str, version: &str) {
    let parsed = ParsedRange::parse(range).unwrap_or_else(|e| panic!("parse({range:?}): {e}"));
    let v = Version::parse(version).unwrap_or_else(|e| panic!("version({version:?}): {e}"));
    assert!(parsed.satisfies(&v), "{version:?} should satisfy {range:?}");
}

#[test]
fn hyphen_ranges() {
    check("1.0.0 - 2.0.0", "1.2.3");
    check("1.2.3+asdf - 2.4.3+asdf", "1.2.3");
}

#[test]
fn build_metadata_in_caret() {
    check("^1.2.3+build", "1.2.3");
    check("^1.2.3+build", "1.3.0");
}

#[test]
fn exact_version() {
    check("1.0.0", "1.0.0");
}

#[test]
fn gte_operator() {
    check(">=1.0.0", "1.0.0");
    check(">=1.0.0", "1.0.1");
    check(">=1.0.0", "1.1.0");
    check(">=0.1.97", "0.1.97");
    check(">=1", "1.0.0");
    check(">=1.2", "1.2.8");
}

#[test]
fn gt_operator() {
    check(">1.0.0", "1.0.1");
    check(">1.0.0", "1.1.0");
}

#[test]
fn lte_operator() {
    check("<=2.0.0", "2.0.0");
    check("<=2.0.0", "1.9999.9999");
    check("<=2.0.0", "0.2.9");
}

#[test]
fn lt_operator() {
    check("<2.0.0", "1.9999.9999");
    check("<2.0.0", "0.2.9");
    check("<1.2", "1.1.1");
    check("<1", "0.0.0");
}

#[test]
fn wildcard_ranges() {
    check("*", "1.2.3");
    check("", "1.0.0");
    check(">=*", "0.2.4");
    check("x", "1.2.3");
}

#[test]
fn or_ranges() {
    check("0.1.20 || 1.2.4", "1.2.4");
    check(">=0.2.3 || <0.0.1", "0.0.0");
    check(">=0.2.3 || <0.0.1", "0.2.3");
    check(">=0.2.3 || <0.0.1", "0.2.4");
    check("||", "1.3.4");
}

#[test]
fn x_ranges() {
    check("2.x.x", "2.1.3");
    check("1.2.x", "1.2.3");
    check("1.2.x || 2.x", "2.1.3");
    check("1.2.x || 2.x", "1.2.3");
    check("2.*.*", "2.1.3");
    check("1.2.*", "1.2.3");
    check("1.2.* || 2.*", "2.1.3");
    check("1.2.* || 2.*", "1.2.3");
    check("2", "2.1.2");
    check("2.3", "2.3.1");
    check("2.x", "2.0.0");
}

#[test]
fn tilde_ranges() {
    check("~0.0.1", "0.0.1");
    check("~0.0.1", "0.0.2");
    check("~2.4", "2.4.0");
    check("~2.4", "2.4.5");
    check("~>3.2.1", "3.2.2");
    check("~1", "1.2.3");
    check("~>1", "1.2.3");
    check("~> 1", "1.2.3");
    check("~1.0", "1.0.2");
    check("~ 1.0", "1.0.2");
    check("~ 1.0.3", "1.0.12");
    check("~x", "0.0.9");
    check("~2", "2.0.9");
}

#[test]
fn tilde_with_v_prefix_and_prerelease() {
    // Prerelease in range bound is stripped; 0.5.4 and 0.5.5 both satisfy ~0.5.4
    check("~v0.5.4-pre", "0.5.5");
    check("~v0.5.4-pre", "0.5.4");
}

#[test]
fn caret_ranges() {
    check("^1.2.3", "1.8.1");
    check("^0.1.2", "0.1.2");
    check("^0.1", "0.1.2");
    check("^0.0.1", "0.0.1");
    check("^1.2", "1.4.2");
    check("^1.2 ^1", "1.4.2");
    check("^x", "1.2.3");
}

#[test]
fn operator_with_x_ranges() {
    check("=0.7.x", "0.7.2");
    check("<=0.7.x", "0.7.2");
    check(">=0.7.x", "0.7.2");
    check("<=0.7.x", "0.6.2");
    check("<=7.x", "7.9.9");
}

#[test]
fn combined_comparators() {
    check("~1.2.1 >=1.2.3", "1.2.3");
    check("~1.2.1 =1.2.3", "1.2.3");
    check("~1.2.1 1.2.3", "1.2.3");
    check("~1.2.1 >=1.2.3 1.2.3", "1.2.3");
    check("~1.2.1 1.2.3 >=1.2.3", "1.2.3");
    check(">=1.2.1 1.2.3", "1.2.3");
    check("1.2.3 >=1.2.1", "1.2.3");
    check(">=1.2.3 >=1.2.1", "1.2.3");
    check(">=1.2.1 >=1.2.3", "1.2.3");
}

#[test]
fn x_range_with_hyphen() {
    check("x - 1.0.0", "0.9.7");
    check("x - 1.x", "0.9.7");
    check("1.0.0 - x", "1.9.7");
    check("1.x - x", "1.9.7");
}

// --- Whitespace normalization (from upstream) ---

#[test]
fn gte_with_spaces() {
    check(">= 1.0.0", "1.0.0");
    check(">=  1.0.0", "1.0.1");
    check(">=   1.0.0", "1.1.0");
}

#[test]
fn gt_with_spaces() {
    check("> 1.0.0", "1.0.1");
    check(">  1.0.0", "1.1.0");
}

#[test]
fn lte_with_spaces() {
    check("<=   2.0.0", "2.0.0");
    check("<= 2.0.0", "1.9999.9999");
    check("<=  2.0.0", "0.2.9");
}

#[test]
fn lt_with_spaces() {
    check("<    2.0.0", "1.9999.9999");
    check("<\t2.0.0", "0.2.9");
}

#[test]
fn partial_with_spaces() {
    check(">= 1", "1.0.0");
    check("< 1.2", "1.1.1");
}

// --- Prerelease ranges (from upstream, previously skipped) ---

#[test]
fn prerelease_hyphen_ranges() {
    check("1.2.3-pre+asdf - 2.4.3-pre+asdf", "1.2.3-pre.2");
    check("1.2.3-pre+asdf - 2.4.3-pre+asdf", "2.4.3-alpha");
}

#[test]
fn prerelease_caret_include() {
    check("^1.2.3-alpha", "1.2.3-pre");
    check("^1.2.0-alpha", "1.2.0-pre");
    check("^0.0.1-alpha", "0.0.1-beta");
    check("^0.0.1-alpha", "0.0.1");
    check("^0.1.1-alpha", "0.1.1-beta");
}
