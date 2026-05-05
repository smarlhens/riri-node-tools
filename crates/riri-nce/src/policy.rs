//! Lifecycle-policy rewrite of `engines.node` ranges.
//!
//! Given a parsed [`LifecycleData`] snapshot already resolved against `today`,
//! [`rewrite_node_range`] applies a [`Policy`] gate to each `||`-separated
//! disjunct of an input range and returns a rewritten range plus a structured
//! breakdown of dropped, bumped, and EOL-flagged disjuncts.

use chrono::NaiveDate;
use riri_node_lifecycle::{LifecycleData, Policy, Status};
use riri_semver_range::{Op, ParsedRange, RangePart, VersionPrecision, split_by_major};
use semver::Version;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RewriteError {
    #[error("invalid semver range: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct PolicyContext<'a> {
    pub data: &'a LifecycleData,
    pub policy: Policy,
    pub today: NaiveDate,
    pub allow_eol: bool,
    pub precision: VersionPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EolWarning {
    pub major: u32,
    pub since: NaiveDate,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyResult {
    pub rewritten: Option<String>,
    pub dropped_disjuncts: Vec<String>,
    pub bumped_disjuncts: Vec<(String, String)>,
    pub eol_warnings: Vec<EolWarning>,
    pub unsatisfiable: bool,
}

/// Rewrites `range` under the given lifecycle policy.
///
/// `ctx.data` MUST have been resolved against `ctx.today` (e.g. via
/// [`LifecycleData::resolve_statuses`] or [`LifecycleData::load_with_cache_override`]).
///
/// # Errors
///
/// Returns [`RewriteError::Parse`] when `range` is not a valid semver range.
pub fn rewrite_node_range(
    range: &str,
    ctx: &PolicyContext<'_>,
) -> Result<PolicyResult, RewriteError> {
    let parsed = ParsedRange::parse(range).map_err(RewriteError::Parse)?;
    let allowed = ctx.data.allowed_majors(ctx.policy);

    let mut new_parts: Vec<RangePart> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut bumped: Vec<(String, String)> = Vec::new();

    for input_part in &parsed.parts {
        let original = humanize_part(input_part, ctx.precision);
        let contributions = if is_wildcard(input_part) {
            expand_from_major(0, &allowed, ctx.policy)
        } else {
            filter_split(input_part, &allowed, ctx.policy)
        };

        if contributions.is_empty() {
            dropped.push(original);
            continue;
        }

        let baseline = if is_wildcard(input_part) {
            Vec::new()
        } else {
            split_by_major(input_part)
        };
        if contributions != baseline {
            let rewritten_part = humanize_parts(&contributions, ctx.precision);
            bumped.push((original, rewritten_part));
        }
        new_parts.extend(contributions);
    }

    if new_parts.is_empty() {
        return Ok(PolicyResult {
            rewritten: None,
            dropped_disjuncts: dropped,
            bumped_disjuncts: bumped,
            eol_warnings: Vec::new(),
            unsatisfiable: true,
        });
    }

    let mut eol_warnings: Vec<EolWarning> = Vec::new();
    if !ctx.allow_eol {
        for part in &new_parts {
            let Ok(major) = u32::try_from(part.min.major) else {
                continue;
            };
            if let Some(info) = ctx.data.majors.get(&major)
                && info.status == Status::EndOfLife
                && !eol_warnings.iter().any(|w| w.major == major)
            {
                eol_warnings.push(EolWarning {
                    major,
                    since: info.end,
                });
            }
        }
    }

    let rebuilt = ParsedRange { parts: new_parts };
    let rewritten = rebuilt.humanize_with(ctx.precision);

    Ok(PolicyResult {
        rewritten: Some(rewritten),
        dropped_disjuncts: dropped,
        bumped_disjuncts: bumped,
        eol_warnings,
        unsatisfiable: false,
    })
}

fn filter_split(input_part: &RangePart, allowed: &[u32], policy: Policy) -> Vec<RangePart> {
    if input_part.max.is_none() {
        let Ok(min_major) = u32::try_from(input_part.min.major) else {
            return Vec::new();
        };
        if matches!(policy, Policy::Any | Policy::Stable) {
            let Some(smallest) = allowed.iter().copied().find(|&m| m >= min_major) else {
                return Vec::new();
            };
            if smallest == min_major {
                return vec![input_part.clone()];
            }
            return vec![RangePart {
                min: Version::new(u64::from(smallest), 0, 0),
                min_op: Op::Gte,
                max: None,
                max_op: None,
            }];
        }
        return expand_from_major(min_major, allowed, policy);
    }

    split_by_major(input_part)
        .into_iter()
        .filter(|sub| {
            u32::try_from(sub.min.major)
                .ok()
                .is_some_and(|m| allowed.contains(&m))
        })
        .collect()
}

/// Expands `>=min_major.0.0` into the allowed-major caret list under `policy`.
///
/// Under `Supported`, the highest allowed caret is replaced with an open `>=M.0.0`
/// to keep the range forward-compatible with the next current major. Under `Lts` and
/// `Maintenance`, every contribution stays caret-bounded.
fn expand_from_major(min_major: u32, allowed: &[u32], policy: Policy) -> Vec<RangePart> {
    let mut filtered: Vec<u32> = allowed
        .iter()
        .copied()
        .filter(|m| *m >= min_major)
        .collect();
    if filtered.is_empty() {
        return Vec::new();
    }
    let keep_open_tail = matches!(policy, Policy::Supported);
    let last = filtered.pop().expect("non-empty after is_empty check");
    let mut parts: Vec<RangePart> = filtered.into_iter().map(caret_from_major).collect();
    if keep_open_tail {
        parts.push(RangePart {
            min: Version::new(u64::from(last), 0, 0),
            min_op: Op::Gte,
            max: None,
            max_op: None,
        });
    } else {
        parts.push(caret_from_major(last));
    }
    parts
}

fn is_wildcard(part: &RangePart) -> bool {
    part.max.is_none()
        && part.min_op == Op::Gte
        && part.min.major == 0
        && part.min.minor == 0
        && part.min.patch == 0
}

fn caret_from_major(m: u32) -> RangePart {
    RangePart {
        min: Version::new(u64::from(m), 0, 0),
        min_op: Op::Gte,
        max: Some(Version::new(u64::from(m) + 1, 0, 0)),
        max_op: Some(Op::Lt),
    }
}

fn humanize_part(part: &RangePart, precision: VersionPrecision) -> String {
    ParsedRange {
        parts: vec![part.clone()],
    }
    .humanize_with(precision)
}

fn humanize_parts(parts: &[RangePart], precision: VersionPrecision) -> String {
    ParsedRange {
        parts: parts.to_vec(),
    }
    .humanize_with(precision)
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
                "18": {
                    "major": 18, "status": "end-of-life", "lts_codename": "Hydrogen",
                    "start": "2022-04-19", "lts_start": "2022-10-25",
                    "maintenance_start": "2023-10-18", "end": "2025-04-30",
                    "lowest": "18.0.0", "highest": "18.20.5",
                    "npm_at_lowest": "8.5.0", "npm_at_highest": "10.8.2",
                    "npm_min_in_major": "8.5.0",
                    "releases": [{"version": "18.0.0", "npm": "8.5.0", "date": "2022-04-19"}]
                },
                "20": {
                    "major": 20, "status": "maintenance", "lts_codename": "Iron",
                    "start": "2023-04-18", "lts_start": "2023-10-24",
                    "maintenance_start": "2024-10-22", "end": "2026-04-30",
                    "lowest": "20.0.0", "highest": "20.19.5",
                    "npm_at_lowest": "9.6.4", "npm_at_highest": "10.8.2",
                    "npm_min_in_major": "9.6.4",
                    "releases": [{"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"}]
                },
                "22": {
                    "major": 22, "status": "active", "lts_codename": "Jod",
                    "start": "2024-04-24", "lts_start": "2024-10-29",
                    "maintenance_start": "2026-10-21", "end": "2027-04-30",
                    "lowest": "22.0.0", "highest": "22.12.0",
                    "npm_at_lowest": "10.5.1", "npm_at_highest": "10.9.1",
                    "npm_min_in_major": "10.5.1",
                    "releases": [{"version": "22.0.0", "npm": "10.5.1", "date": "2024-04-24"}]
                },
                "23": {
                    "major": 23, "status": "end-of-life", "lts_codename": null,
                    "start": "2024-10-15", "lts_start": null, "maintenance_start": null,
                    "end": "2025-06-01", "lowest": "23.0.0", "highest": "23.11.1",
                    "npm_at_lowest": "10.9.0", "npm_at_highest": "10.9.2",
                    "npm_min_in_major": "10.9.0",
                    "releases": [{"version": "23.0.0", "npm": "10.9.0", "date": "2024-10-15"}]
                },
                "24": {
                    "major": 24, "status": "active", "lts_codename": "Krypton",
                    "start": "2025-04-22", "lts_start": "2025-10-28",
                    "maintenance_start": "2027-10-20", "end": "2028-04-30",
                    "lowest": "24.0.0", "highest": "24.6.0",
                    "npm_at_lowest": "11.0.0", "npm_at_highest": "11.5.1",
                    "npm_min_in_major": "11.0.0",
                    "releases": [{"version": "24.0.0", "npm": "11.0.0", "date": "2025-04-22"}]
                },
                "25": {
                    "major": 25, "status": "current", "lts_codename": null,
                    "start": "2025-10-21", "lts_start": null, "maintenance_start": null,
                    "end": "2026-06-01", "lowest": "25.0.0", "highest": "25.2.0",
                    "npm_at_lowest": "11.5.0", "npm_at_highest": "11.5.1",
                    "npm_min_in_major": "11.5.0",
                    "releases": [{"version": "25.0.0", "npm": "11.5.0", "date": "2025-10-21"}]
                }
            }
        }"#;
        let mut data = LifecycleData::parse(json).expect("parse fixture");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2026, 4, 29).expect("date"));
        data
    }

    fn ctx(data: &LifecycleData, policy: Policy) -> PolicyContext<'_> {
        PolicyContext {
            data,
            policy,
            today: NaiveDate::from_ymd_opt(2026, 4, 29).expect("date"),
            allow_eol: false,
            precision: VersionPrecision::Major,
        }
    }

    #[test]
    fn fixture_resolves_expected_statuses() {
        let data = fixture();
        assert_eq!(data.majors[&18].status, Status::EndOfLife);
        assert_eq!(data.majors[&20].status, Status::Maintenance);
        assert_eq!(data.majors[&22].status, Status::Active);
        assert_eq!(data.majors[&23].status, Status::EndOfLife);
        assert_eq!(data.majors[&24].status, Status::Active);
        assert_eq!(data.majors[&25].status, Status::Current);
    }

    #[test]
    fn lts_keeps_caret_on_allowed_major_and_lifts_open_lower_bound() {
        let data = fixture();
        let result =
            rewrite_node_range("^20.19.0 || ^22.12.0 || >=23.0.0", &ctx(&data, Policy::Lts))
                .expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^20.19 || ^22.12 || ^24"));
        assert!(result.dropped_disjuncts.is_empty());
        assert_eq!(result.bumped_disjuncts.len(), 1);
        assert_eq!(result.bumped_disjuncts[0].0, ">=23");
        assert_eq!(result.bumped_disjuncts[0].1, "^24");
        assert!(!result.unsatisfiable);
        assert!(result.eol_warnings.is_empty());
    }

    #[test]
    fn supported_drops_caret_on_eol_only_disjunct() {
        let data = fixture();
        let result =
            rewrite_node_range("^18.0.0", &ctx(&data, Policy::Supported)).expect("rewrite");
        assert!(result.unsatisfiable);
        assert_eq!(result.rewritten, None);
        assert_eq!(result.dropped_disjuncts, vec!["^18".to_string()]);
    }

    #[test]
    fn any_keeps_eol_caret_and_emits_warning() {
        let data = fixture();
        let result = rewrite_node_range("^18.0.0", &ctx(&data, Policy::Any)).expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^18"));
        assert_eq!(result.eol_warnings.len(), 1);
        assert_eq!(result.eol_warnings[0].major, 18);
        assert_eq!(
            result.eol_warnings[0].since,
            NaiveDate::from_ymd_opt(2025, 4, 30).expect("date")
        );
    }

    #[test]
    fn allow_eol_suppresses_warning_without_widening_policy() {
        let data = fixture();
        let mut c = ctx(&data, Policy::Any);
        c.allow_eol = true;
        let result = rewrite_node_range("^18.0.0", &c).expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^18"));
        assert!(result.eol_warnings.is_empty());
    }

    #[test]
    fn lts_lifts_open_eol_lower_bound() {
        let data = fixture();
        let result = rewrite_node_range(">=18.0.0", &ctx(&data, Policy::Lts)).expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^20 || ^22 || ^24"));
        assert_eq!(
            result.bumped_disjuncts,
            vec![(">=18".to_string(), "^20 || ^22 || ^24".to_string())]
        );
    }

    #[test]
    fn lts_narrows_compound_bounded_dropping_disallowed_intermediate_majors() {
        let data = fixture();
        let result =
            rewrite_node_range(">=20.0.0 <22.0.0", &ctx(&data, Policy::Lts)).expect("rewrite");
        // Input spans 20 (Maintenance, allowed) and 21 (no data → not allowed).
        // Result keeps only the 20 sub-clause.
        assert_eq!(result.rewritten.as_deref(), Some("^20"));
        assert!(result.dropped_disjuncts.is_empty());
        // Humanizer always splits cross-major ranges, so the "from" side reflects
        // the per-major decomposition rather than the literal input.
        assert_eq!(
            result.bumped_disjuncts,
            vec![("^20 || ^21".to_string(), "^20".to_string())]
        );
    }

    #[test]
    fn lts_drops_compound_bounded_with_no_allowed_major_in_range() {
        let data = fixture();
        let result =
            rewrite_node_range(">=23.0.0 <24.0.0", &ctx(&data, Policy::Lts)).expect("rewrite");
        assert!(result.unsatisfiable);
        assert_eq!(result.dropped_disjuncts, vec!["^23".to_string()]);
    }

    #[test]
    fn lts_expands_wildcard_into_or_of_carets() {
        let data = fixture();
        let result = rewrite_node_range("*", &ctx(&data, Policy::Lts)).expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^20 || ^22 || ^24"));
    }

    #[test]
    fn maintenance_keeps_only_maintenance_majors() {
        let data = fixture();
        let result = rewrite_node_range("^20.19.0 || ^22.12.0", &ctx(&data, Policy::Maintenance))
            .expect("rewrite");
        assert_eq!(result.rewritten.as_deref(), Some("^20.19"));
        assert_eq!(result.dropped_disjuncts, vec!["^22.12".to_string()]);
    }

    #[test]
    fn parse_error_propagates() {
        let data = fixture();
        let err =
            rewrite_node_range("not a range", &ctx(&data, Policy::Lts)).expect_err("invalid range");
        assert!(matches!(err, RewriteError::Parse(_)));
    }
}
