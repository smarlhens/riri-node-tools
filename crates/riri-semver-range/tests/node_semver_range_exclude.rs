#![allow(clippy::tests_outside_test_module)]
//! Ported from node-semver test/fixtures/range-exclude.js
//! Tests that version should NOT satisfy the given range.
//! Skipped: loose mode, includePrerelease, invalid/non-string versions,
//!          prerelease version matching differences.

use riri_semver_range::ParsedRange;
use semver::Version;

fn check_excludes(range: &str, version: &str) {
    let parsed = ParsedRange::parse(range).unwrap_or_else(|e| panic!("parse({range:?}): {e}"));
    let v = Version::parse(version).unwrap_or_else(|e| panic!("version({version:?}): {e}"));
    assert!(
        !parsed.satisfies(&v),
        "{version:?} should NOT satisfy {range:?}"
    );
}

#[test]
fn hyphen_ranges() {
    check_excludes("1.0.0 - 2.0.0", "2.2.3");
}

#[test]
fn build_metadata_excludes() {
    check_excludes("^1.2.3+build", "2.0.0");
    check_excludes("^1.2.3+build", "1.2.0");
}

#[test]
fn caret_excludes() {
    check_excludes("^1.2.3", "1.2.2");
    check_excludes("^1.2", "1.1.9");
    check_excludes("^0.0.1", "0.0.2");
}

#[test]
fn exact_version() {
    check_excludes("1.0.0", "1.0.1");
}

#[test]
fn gte_excludes() {
    check_excludes(">=1.0.0", "0.0.0");
    check_excludes(">=1.0.0", "0.0.1");
    check_excludes(">=1.0.0", "0.1.0");
    check_excludes(">=0.1.97", "0.1.93");
    check_excludes(">=1.2", "1.1.1");
}

#[test]
fn gt_excludes() {
    check_excludes(">1.0.0", "0.0.1");
    check_excludes(">1.0.0", "0.1.0");
    check_excludes(">1.2", "1.2.8");
}

#[test]
fn lte_excludes() {
    check_excludes("<=2.0.0", "3.0.0");
    check_excludes("<=2.0.0", "2.9999.9999");
    check_excludes("<=2.0.0", "2.2.9");
}

#[test]
fn lt_excludes() {
    check_excludes("<2.0.0", "2.9999.9999");
    check_excludes("<2.0.0", "2.2.9");
    check_excludes("<1", "1.0.0");
}

#[test]
fn or_excludes() {
    check_excludes("0.1.20 || 1.2.4", "1.2.3");
    check_excludes(">=0.2.3 || <0.0.1", "0.0.3");
    check_excludes(">=0.2.3 || <0.0.1", "0.2.2");
}

#[test]
fn x_range_excludes() {
    check_excludes("2.x.x", "1.1.3");
    check_excludes("2.x.x", "3.1.3");
    check_excludes("1.2.x", "1.3.3");
    check_excludes("1.2.x || 2.x", "3.1.3");
    check_excludes("1.2.x || 2.x", "1.1.3");
    check_excludes("2.*.*", "1.1.3");
    check_excludes("2.*.*", "3.1.3");
    check_excludes("1.2.*", "1.3.3");
    check_excludes("1.2.* || 2.*", "3.1.3");
    check_excludes("1.2.* || 2.*", "1.1.3");
    check_excludes("2", "1.1.2");
    check_excludes("2.3", "2.4.1");
}

#[test]
fn tilde_excludes() {
    check_excludes("~2.4", "2.5.0");
    check_excludes("~2.4", "2.3.9");
    check_excludes("~>3.2.1", "3.3.2");
    check_excludes("~>3.2.1", "3.2.0");
    check_excludes("~1", "0.2.3");
    check_excludes("~>1", "2.2.3");
    check_excludes("~1.0", "1.1.0");
}

#[test]
fn tilde_prerelease_excludes() {
    // ~v0.5.4-beta: range becomes ~0.5.4, so 0.5.4-alpha < 0.5.4 → excluded
    check_excludes("~v0.5.4-beta", "0.5.4-alpha");
}

#[test]
fn operator_x_range_excludes() {
    check_excludes("=0.7.x", "0.8.2");
    check_excludes(">=0.7.x", "0.6.2");
    check_excludes("<0.7.x", "0.7.2");
}

#[test]
fn prerelease_not_matched_without_flag() {
    // Node-semver prerelease filter: prerelease versions are excluded when
    // no bound in the range has a prerelease on the same [M.m.p] tuple.
    check_excludes("=1.2.3", "1.2.3-beta");
    check_excludes("<1.2.3", "1.2.3-beta");
    check_excludes("^1.2.3", "2.0.0-alpha");
    check_excludes("^0.0.1", "0.0.2-alpha");
    check_excludes("^1.2.3", "1.2.3-pre");
    check_excludes("^1.2", "1.2.0-pre");
    check_excludes(">1.2", "1.3.0-beta");
    check_excludes("<=1.2.3", "1.2.3-beta");
    check_excludes("=0.7.x", "0.7.0-asdf");
    check_excludes(">=0.7.x", "0.7.0-asdf");
    check_excludes("<=0.7.x", "0.7.0-asdf");
    check_excludes("^1.2.3", "1.2.3-beta");
}

#[test]
fn prerelease_hyphen_exclude() {
    check_excludes("1.2.3+asdf - 2.4.3+asdf", "1.2.3-pre.2");
    check_excludes("1.2.3+asdf - 2.4.3+asdf", "2.4.3-alpha");
}

#[test]
fn prerelease_x_range_excludes() {
    check_excludes("1.1.x", "1.1.0-a");
    check_excludes("1.1.x", "1.2.0-a");
    check_excludes("1.x", "1.0.0-a");
    check_excludes("1.x", "1.1.0-a");
    check_excludes("1.x", "1.2.0-a");
}

#[test]
fn prerelease_tilde_excludes() {
    check_excludes("~0.0.1", "0.1.0-alpha");
}

#[test]
fn gte_lt_excludes() {
    check_excludes(">=1.0.0 <1.1.0", "1.1.0");
    check_excludes(">=1.0.0 <1.1.0", "1.1.0-pre");
}
