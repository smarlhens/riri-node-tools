//! npm engine floor derivation from a node engine range.
//!
//! Derives the lowest npm version compatible with every OR-disjunct of an
//! `engines.node` range, then decides whether the project's declared
//! `engines.npm` constraint needs to be bumped.

use riri_node_lifecycle::LifecycleData;
use riri_semver_range::{ParsedRange, VersionPrecision};
use semver::Version;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NpmBumpError {
    #[error("invalid node range: {0}")]
    InvalidNodeRange(String),
    #[error("no disjunct of node range maps to a known npm version")]
    NoUsableDisjunct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpmBumpResult {
    pub target_floor: Version,
    pub apply: bool,
    pub reason: String,
}

/// Computes the conservative npm floor for `node_range` under `data`.
///
/// For each OR-disjunct, the lower bound version `(M, m, p)` is mapped to npm:
///   1. Exact release `M.m.p` known → its bundled npm.
///   2. Else `data.majors[M].npm_min_in_major` (conservative).
///
/// The minimum across all disjuncts is the floor. `precision` is currently
/// unused at the type level (the returned `Version` is always 3-component) and
/// is reserved for future display-time trimming via the humanize layer.
///
/// # Errors
///
/// Returns [`NpmBumpError::InvalidNodeRange`] when `node_range` does not parse,
/// and [`NpmBumpError::NoUsableDisjunct`] when no disjunct maps to known npm
/// data (e.g. all majors absent from `data`).
pub fn derive_npm_floor(
    node_range: &str,
    data: &LifecycleData,
    _precision: VersionPrecision,
) -> Result<Version, NpmBumpError> {
    let parsed = ParsedRange::parse(node_range).map_err(NpmBumpError::InvalidNodeRange)?;

    let mut floor: Option<Version> = None;
    for part in &parsed.parts {
        let Ok(major) = u32::try_from(part.min.major) else {
            continue;
        };
        let Some(info) = data.majors.get(&major) else {
            continue;
        };
        let candidate = info
            .releases
            .iter()
            .find(|r| r.version == part.min)
            .map_or_else(|| info.npm_min_in_major.clone(), |r| r.npm.clone());
        floor = Some(match floor {
            Some(f) if f < candidate => f,
            _ => candidate,
        });
    }

    floor.ok_or(NpmBumpError::NoUsableDisjunct)
}

/// Decides whether the declared npm range needs to be bumped to clear `target`.
///
/// `declared_npm` is the existing `engines.npm` range string from `package.json`,
/// or `None` when absent. The comparison takes the parsed declared range's
/// lowest lower bound and compares against `target`.
///
/// A `declared_npm` that fails to parse is treated as "no declared npm" — the
/// caller will overwrite it with the bumped value.
#[must_use]
pub fn maybe_bump_npm(declared_npm: Option<&str>, target: &Version) -> NpmBumpResult {
    let declared_lower = declared_npm.and_then(|s| {
        ParsedRange::parse(s)
            .ok()
            .and_then(|p| p.parts.first().map(|first| first.min.clone()))
    });

    match declared_lower {
        None => NpmBumpResult {
            target_floor: target.clone(),
            apply: true,
            reason: format!("declared npm missing; setting floor to {target}"),
        },
        Some(declared) if declared < *target => NpmBumpResult {
            target_floor: target.clone(),
            apply: true,
            reason: format!("declared npm {declared} below floor {target}"),
        },
        Some(declared) => NpmBumpResult {
            target_floor: target.clone(),
            apply: false,
            reason: format!("declared npm {declared} already meets floor {target}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riri_node_lifecycle::LifecycleData;

    fn fixture() -> LifecycleData {
        let json = r#"{
            "schema_version": 1,
            "fetched_at": "2026-04-29T00:00:00Z",
            "majors": {
                "20": {
                    "major": 20, "status": "maintenance", "lts_codename": "Iron",
                    "start": "2023-04-18", "lts_start": "2023-10-24",
                    "maintenance_start": "2024-10-22", "end": "2026-04-30",
                    "lowest": "20.0.0", "highest": "20.19.5",
                    "npm_at_lowest": "9.6.4", "npm_at_highest": "10.8.2",
                    "npm_min_in_major": "9.6.4",
                    "releases": [
                        {"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"},
                        {"version": "20.19.5", "npm": "10.8.2", "date": "2025-09-15"}
                    ]
                },
                "22": {
                    "major": 22, "status": "active", "lts_codename": "Jod",
                    "start": "2024-04-24", "lts_start": "2024-10-29",
                    "maintenance_start": "2026-10-21", "end": "2027-04-30",
                    "lowest": "22.0.0", "highest": "22.12.0",
                    "npm_at_lowest": "10.5.1", "npm_at_highest": "10.9.1",
                    "npm_min_in_major": "10.5.1",
                    "releases": [
                        {"version": "22.0.0", "npm": "10.5.1", "date": "2024-04-24"},
                        {"version": "22.12.0", "npm": "10.9.1", "date": "2024-12-03"}
                    ]
                }
            }
        }"#;
        LifecycleData::parse(json).expect("parse fixture")
    }

    fn v(s: &str) -> Version {
        Version::parse(s).expect("semver")
    }

    #[test]
    fn floor_for_single_caret_uses_lowest_npm_in_major() {
        let data = fixture();
        let floor = derive_npm_floor("^20.0.0", &data, VersionPrecision::Major).expect("floor");
        assert_eq!(floor, v("9.6.4"));
    }

    #[test]
    fn floor_for_exact_match_uses_release_npm() {
        let data = fixture();
        let floor = derive_npm_floor(">=22.12.0", &data, VersionPrecision::Major).expect("floor");
        assert_eq!(floor, v("10.9.1"));
    }

    #[test]
    fn floor_for_unknown_release_falls_back_to_npm_min_in_major() {
        let data = fixture();
        // 22.5.0 is not in the releases list — fall back to npm_min_in_major (10.5.1).
        let floor = derive_npm_floor(">=22.5.0", &data, VersionPrecision::Major).expect("floor");
        assert_eq!(floor, v("10.5.1"));
    }

    #[test]
    fn floor_across_disjuncts_takes_minimum() {
        let data = fixture();
        // ^20: 9.6.4 ; ^22: 10.5.1 → min = 9.6.4
        let floor =
            derive_npm_floor("^20.0.0 || ^22.0.0", &data, VersionPrecision::Major).expect("floor");
        assert_eq!(floor, v("9.6.4"));
    }

    #[test]
    fn floor_skips_unknown_majors_silently() {
        let data = fixture();
        // 18 is missing from fixture — skip; 20 contributes 9.6.4.
        let floor =
            derive_npm_floor("^18.0.0 || ^20.0.0", &data, VersionPrecision::Major).expect("floor");
        assert_eq!(floor, v("9.6.4"));
    }

    #[test]
    fn floor_errors_when_no_disjunct_has_data() {
        let data = fixture();
        let err = derive_npm_floor("^14.0.0 || ^16.0.0", &data, VersionPrecision::Major)
            .expect_err("no usable disjunct");
        assert!(matches!(err, NpmBumpError::NoUsableDisjunct));
    }

    #[test]
    fn floor_errors_on_invalid_range() {
        let data = fixture();
        let err = derive_npm_floor("not a range", &data, VersionPrecision::Major)
            .expect_err("invalid range");
        assert!(matches!(err, NpmBumpError::InvalidNodeRange(_)));
    }

    #[test]
    fn maybe_bump_returns_apply_when_declared_npm_missing() {
        let target = v("10.5.1");
        let result = maybe_bump_npm(None, &target);
        assert!(result.apply);
        assert_eq!(result.target_floor, target);
    }

    #[test]
    fn maybe_bump_returns_apply_when_declared_below_target() {
        let target = v("10.5.1");
        let result = maybe_bump_npm(Some(">=9.0.0"), &target);
        assert!(result.apply);
        assert_eq!(result.target_floor, target);
    }

    #[test]
    fn maybe_bump_returns_no_apply_when_declared_meets_target() {
        let target = v("10.5.1");
        let result = maybe_bump_npm(Some(">=10.5.1"), &target);
        assert!(!result.apply);
    }

    #[test]
    fn maybe_bump_returns_no_apply_when_declared_exceeds_target() {
        let target = v("10.5.1");
        let result = maybe_bump_npm(Some(">=11.0.0"), &target);
        assert!(!result.apply);
    }

    #[test]
    fn maybe_bump_with_unparseable_declared_npm_treats_as_missing() {
        let target = v("10.5.1");
        let result = maybe_bump_npm(Some("garbage"), &target);
        assert!(result.apply);
    }
}
