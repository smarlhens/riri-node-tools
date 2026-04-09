#![allow(clippy::tests_outside_test_module)]
//! Property-based tests for `riri-semver-range`.
//!
//! Tests invariants that must hold for all valid inputs, not just specific cases.

use proptest::prelude::*;
use riri_semver_range::{ParsedRange, intersects, is_subset_of, restrictive_range};
use semver::Version;

// --- Strategies ---

/// Generate a random semver version (non-prerelease).
fn arb_version() -> impl Strategy<Value = Version> {
    (0_u64..30, 0_u64..30, 0_u64..30).prop_map(|(ma, mi, pa)| Version::new(ma, mi, pa))
}

/// Generate a random range string from common patterns.
fn arb_range() -> impl Strategy<Value = String> {
    prop_oneof![
        // Caret: ^X.Y.Z
        (0_u64..20, 0_u64..20, 0_u64..20).prop_map(|(ma, mi, pa)| format!("^{ma}.{mi}.{pa}")),
        // Tilde: ~X.Y.Z
        (0_u64..20, 0_u64..20, 0_u64..20).prop_map(|(ma, mi, pa)| format!("~{ma}.{mi}.{pa}")),
        // Gte: >=X.Y.Z
        (0_u64..20, 0_u64..20, 0_u64..20).prop_map(|(ma, mi, pa)| format!(">={ma}.{mi}.{pa}")),
        // Bounded: >=X.Y.Z <(X+N).0.0
        (0_u64..20, 0_u64..20, 0_u64..20, 1_u64..5)
            .prop_map(|(ma, mi, pa, span)| format!(">={ma}.{mi}.{pa} <{}.0.0", ma + span)),
        // Exact
        (0_u64..20, 0_u64..20, 0_u64..20).prop_map(|(ma, mi, pa)| format!("{ma}.{mi}.{pa}")),
        // Wildcard
        Just("*".to_string()),
        // OR of two carets
        (
            0_u64..20,
            0_u64..20,
            0_u64..20,
            0_u64..20,
            0_u64..20,
            0_u64..20
        )
            .prop_map(|(a, b, c, d, e, f)| format!("^{a}.{b}.{c} || ^{d}.{e}.{f}")),
    ]
}

/// Parse a range, skipping if invalid.
fn try_parse(s: &str) -> Option<ParsedRange> {
    ParsedRange::parse(s).ok()
}

// --- Invariant: humanize → parse round-trip ---

proptest! {
    #[test]
    fn humanize_parse_round_trip(range in arb_range(), v in arb_version()) {
        let Some(r) = try_parse(&range) else { return Ok(()); };
        let humanized = r.humanize();
        let Some(r2) = try_parse(&humanized) else {
            return Err(TestCaseError::Fail(
                format!("humanize produced unparseable output: {humanized:?} from {range:?}").into()
            ));
        };
        prop_assert!(
            r.satisfies(&v) == r2.satisfies(&v),
            "round-trip mismatch for {:?} → {:?} at {}", range, humanized, v
        );
    }
}

// --- Invariant: is_subset_of reflexivity ---

proptest! {
    #[test]
    fn subset_reflexive(range in arb_range()) {
        let Some(r) = try_parse(&range) else { return Ok(()); };
        prop_assert!(
            is_subset_of(&r, &r),
            "range should be subset of itself: {:?}", range
        );
    }
}

// --- Invariant: subset implies intersects ---

proptest! {
    #[test]
    fn subset_implies_intersects(r1 in arb_range(), r2 in arb_range()) {
        let (Some(a), Some(b)) = (try_parse(&r1), try_parse(&r2)) else { return Ok(()); };
        if is_subset_of(&a, &b) {
            // If a ⊂ b, they must intersect (unless a is empty/impossible)
            let has_version = (0..30_u64).any(|i| a.satisfies(&Version::new(i, 0, 0)));
            if has_version {
                prop_assert!(
                    intersects(&a, &b),
                    "subset should imply intersects: {:?} ⊂ {:?}", r1, r2
                );
            }
        }
    }
}

// --- Invariant: restrictive_range result is at most as permissive as inputs ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn restrictive_range_tighter(r1 in arb_range(), r2 in arb_range(), v in arb_version()) {
        let (Some(a), Some(b)) = (try_parse(&r1), try_parse(&r2)) else { return Ok(()); };
        let result = restrictive_range(&a, &b);
        if result.satisfies(&v) {
            prop_assert!(
                a.satisfies(&v) || b.satisfies(&v),
                "restrictive_range result satisfies {} but neither input does: {:?}, {:?}", v, r1, r2
            );
        }
    }
}

// --- Invariant: restrictive_range is subset of both (when they intersect) ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn restrictive_range_subset_when_intersecting(r1 in arb_range(), r2 in arb_range(), v in arb_version()) {
        let (Some(a), Some(b)) = (try_parse(&r1), try_parse(&r2)) else { return Ok(()); };
        if intersects(&a, &b) {
            let result = restrictive_range(&a, &b);
            if result.satisfies(&v) {
                prop_assert!(
                    a.satisfies(&v),
                    "restrictive result satisfies {} but r1 doesn't: {:?} ∩ {:?}", v, r1, r2
                );
                prop_assert!(
                    b.satisfies(&v),
                    "restrictive result satisfies {} but r2 doesn't: {:?} ∩ {:?}", v, r1, r2
                );
            }
        }
    }
}

// --- Invariant: no-intersection returns r1 (base) ---

proptest! {
    #[test]
    fn restrictive_no_intersect_returns_base(r1 in arb_range(), r2 in arb_range(), v in arb_version()) {
        let (Some(a), Some(b)) = (try_parse(&r1), try_parse(&r2)) else { return Ok(()); };
        if !intersects(&a, &b) {
            let result = restrictive_range(&a, &b);
            prop_assert!(
                result.satisfies(&v) == a.satisfies(&v),
                "no-intersection should return r1: {:?} vs {:?} at {}", r1, r2, v
            );
        }
    }
}

// --- Invariant: satisfies matches nodejs-semver for non-prerelease ---

proptest! {
    #[test]
    fn satisfies_matches_nodejs_semver(range in arb_range(), v in arb_version()) {
        let Some(our_range) = try_parse(&range) else { return Ok(()); };
        let v_str = format!("{v}");
        let Ok(njs_range) = nodejs_semver::Range::parse(&range) else { return Ok(()); };
        let Ok(njs_version) = nodejs_semver::Version::parse(&v_str) else { return Ok(()); };

        let ours = our_range.satisfies(&v);
        let theirs = njs_range.satisfies(&njs_version);
        prop_assert!(
            ours == theirs,
            "satisfies mismatch for {:?} @ {}: ours={}, nodejs-semver={}", range, v, ours, theirs
        );
    }
}
