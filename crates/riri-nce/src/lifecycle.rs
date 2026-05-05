//! Lifecycle pipeline pass on top of [`check_engines`].
//!
//! Wraps [`check_engines`] with two extra steps:
//!   1. Rewrite `engines.node` under a [`Policy`] gate.
//!   2. Optionally bump `engines.npm` to the floor required by the rewritten node range.

use crate::compute::{CheckEnginesInput, CheckEnginesOutput, EngineRangeToSet, check_engines};
use crate::npm_bump::{NpmBumpResult, derive_npm_floor, maybe_bump_npm};
use crate::policy::{EolWarning, PolicyContext, PolicyResult, RewriteError, rewrite_node_range};
use chrono::{DateTime, NaiveDate, Utc};
use riri_common::EngineConstraintKey;
use riri_node_lifecycle::{LifecycleData, Policy};
use riri_semver_range::{ParsedRange, VersionPrecision};

#[derive(Debug, Clone)]
pub struct LifecycleConfig<'a> {
    pub data: &'a LifecycleData,
    pub policy: Policy,
    pub today: NaiveDate,
    pub allow_eol: bool,
    pub bump_npm: bool,
    pub npm_precision: VersionPrecision,
}

#[derive(Debug, Clone, Default)]
pub struct LifecycleOutput {
    pub policy: Option<Policy>,
    pub data_fetched_at: Option<DateTime<Utc>>,
    pub warnings: Vec<EolWarning>,
    pub dropped_disjuncts: Vec<String>,
    pub bumped_disjuncts: Vec<(String, String)>,
    pub unsatisfiable: bool,
    pub npm_bump: Option<NpmBumpResult>,
}

/// Runs [`check_engines`] then applies the lifecycle pass and (optionally) the
/// npm coupling pass.
///
/// # Errors
///
/// Returns [`RewriteError`] when the rewritten node range fails to parse.
pub fn check_engines_with_lifecycle(
    input: &CheckEnginesInput<'_>,
    cfg: &LifecycleConfig<'_>,
) -> Result<(CheckEnginesOutput, LifecycleOutput), RewriteError> {
    let mut output = check_engines(input);
    let mut lifecycle = LifecycleOutput {
        policy: Some(cfg.policy),
        data_fetched_at: Some(cfg.data.fetched_at),
        ..LifecycleOutput::default()
    };

    let node_to = output
        .computed_engines
        .get(&EngineConstraintKey::Node)
        .cloned();
    if let Some(node_to) = node_to {
        let ctx = PolicyContext {
            data: cfg.data,
            policy: cfg.policy,
            today: cfg.today,
            allow_eol: cfg.allow_eol,
            precision: input.precision,
        };
        let pr = rewrite_node_range(&node_to, &ctx)?;
        apply_policy_result(&pr, &node_to, &mut output, input);
        lifecycle.warnings = pr.eol_warnings;
        lifecycle.dropped_disjuncts = pr.dropped_disjuncts;
        lifecycle.bumped_disjuncts = pr.bumped_disjuncts;
        lifecycle.unsatisfiable = pr.unsatisfiable;
    }

    let npm_filtered_out = !input.filter_engines.is_empty()
        && !input.filter_engines.contains(&EngineConstraintKey::Npm);
    if cfg.bump_npm
        && !npm_filtered_out
        && let Some(node_final) = output
            .computed_engines
            .get(&EngineConstraintKey::Node)
            .cloned()
        && let Ok(target) = derive_npm_floor(&node_final, cfg.data, cfg.npm_precision)
    {
        let declared = output
            .computed_engines
            .get(&EngineConstraintKey::Npm)
            .cloned();
        let bump = maybe_bump_npm(declared.as_deref(), &target);
        if bump.apply {
            let new_npm = format_npm_floor(&bump.target_floor, cfg.npm_precision);
            apply_npm_bump(&new_npm, &mut output, input);
        }
        lifecycle.npm_bump = Some(bump);
    }

    Ok((output, lifecycle))
}

fn apply_policy_result(
    pr: &PolicyResult,
    previous_node: &str,
    output: &mut CheckEnginesOutput,
    input: &CheckEnginesInput<'_>,
) {
    let Some(rewritten) = pr.rewritten.clone() else {
        return;
    };
    if rewritten == previous_node {
        return;
    }
    output
        .computed_engines
        .insert(EngineConstraintKey::Node, rewritten.clone());
    upsert_change(
        &mut output.engines_range_to_set,
        EngineConstraintKey::Node,
        original_from(input, EngineConstraintKey::Node),
        rewritten,
    );
}

fn format_npm_floor(target: &semver::Version, precision: VersionPrecision) -> String {
    let raw = format!(">={target}");
    ParsedRange::parse(&raw)
        .map(|r| r.humanize_with(precision))
        .unwrap_or(raw)
}

fn apply_npm_bump(new_npm: &str, output: &mut CheckEnginesOutput, input: &CheckEnginesInput<'_>) {
    output
        .computed_engines
        .insert(EngineConstraintKey::Npm, new_npm.to_string());
    upsert_change(
        &mut output.engines_range_to_set,
        EngineConstraintKey::Npm,
        original_from(input, EngineConstraintKey::Npm),
        new_npm.to_string(),
    );
}

fn original_from(input: &CheckEnginesInput<'_>, key: EngineConstraintKey) -> String {
    input
        .package_engines
        .and_then(|pkg| pkg.get(&key.to_string()).cloned())
        .unwrap_or_else(|| "*".to_string())
}

fn upsert_change(
    list: &mut Vec<EngineRangeToSet>,
    key: EngineConstraintKey,
    from: String,
    new_to: String,
) {
    if let Some(existing) = list.iter_mut().find(|e| e.engine == key) {
        existing.range_to_set = new_to;
    } else if from != new_to {
        list.push(EngineRangeToSet {
            engine: key,
            range: from,
            range_to_set: new_to,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riri_semver_range::VersionPrecision;
    use std::collections::HashMap;

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
                }
            }
        }"#;
        let mut data = LifecycleData::parse(json).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2026, 4, 29).expect("date"));
        data
    }

    fn pkg_engines(node: Option<&str>, npm: Option<&str>) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(n) = node {
            map.insert("node".into(), n.to_string());
        }
        if let Some(n) = npm {
            map.insert("npm".into(), n.to_string());
        }
        map
    }

    fn cfg(data: &LifecycleData, policy: Policy, bump_npm: bool) -> LifecycleConfig<'_> {
        LifecycleConfig {
            data,
            policy,
            today: NaiveDate::from_ymd_opt(2026, 4, 29).expect("date"),
            allow_eol: false,
            bump_npm,
            npm_precision: VersionPrecision::Major,
        }
    }

    #[test]
    fn lifecycle_lifts_eol_node_lower_bound_under_supported() {
        let data = fixture();
        let pkg = pkg_engines(Some(">=18.0.0"), None);
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node],
            precision: VersionPrecision::Major,
        };
        let (output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Supported, false))
                .expect("lifecycle");

        assert_eq!(
            output.computed_engines[&EngineConstraintKey::Node],
            "^20 || >=22"
        );
        assert!(lifecycle.warnings.is_empty());
        assert_eq!(
            lifecycle.bumped_disjuncts,
            vec![(">=18".to_string(), "^20 || >=22".to_string())]
        );
    }

    #[test]
    fn lifecycle_bumps_npm_floor_to_match_node_under_supported() {
        let data = fixture();
        let pkg = pkg_engines(Some(">=20.0.0"), Some(">=8.0.0"));
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node, EngineConstraintKey::Npm],
            precision: VersionPrecision::Major,
        };
        let (output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Supported, true))
                .expect("lifecycle");

        // node>=20 → npm floor = 9.6.4 (npm_min_in_major for 20). Declared 8.0.0 < 9.6.4 → bump.
        assert_eq!(
            output.computed_engines[&EngineConstraintKey::Npm],
            ">=9.6.4"
        );
        let bump = lifecycle.npm_bump.expect("bump");
        assert!(bump.apply);
        assert_eq!(bump.target_floor.to_string(), "9.6.4");
    }

    #[test]
    fn lifecycle_skips_npm_bump_when_disabled() {
        let data = fixture();
        let pkg = pkg_engines(Some(">=20.0.0"), Some(">=8.0.0"));
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node, EngineConstraintKey::Npm],
            precision: VersionPrecision::Major,
        };
        let (output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Supported, false))
                .expect("lifecycle");

        // bump_npm=false → npm should remain unchanged at the original ">=8.0.0".
        assert_eq!(output.computed_engines[&EngineConstraintKey::Npm], ">=8");
        assert!(lifecycle.npm_bump.is_none());
    }

    #[test]
    fn lifecycle_emits_eol_warning_under_any() {
        let data = fixture();
        let pkg = pkg_engines(Some("^18.0.0"), None);
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node],
            precision: VersionPrecision::Major,
        };
        let (_output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Any, false))
                .expect("lifecycle");

        assert_eq!(lifecycle.warnings.len(), 1);
        assert_eq!(lifecycle.warnings[0].major, 18);
    }

    #[test]
    fn lifecycle_marks_unsatisfiable_when_all_disjuncts_dropped() {
        let data = fixture();
        let pkg = pkg_engines(Some("^18.0.0"), None);
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node],
            precision: VersionPrecision::Major,
        };
        let (_output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Lts, false))
                .expect("lifecycle");

        assert!(lifecycle.unsatisfiable);
        assert_eq!(lifecycle.dropped_disjuncts, vec!["^18".to_string()]);
    }

    #[test]
    fn lifecycle_passes_through_when_node_already_complies() {
        let data = fixture();
        let pkg = pkg_engines(Some("^22.0.0"), Some(">=11.0.0"));
        let input = CheckEnginesInput {
            lockfile_entries: Vec::new(),
            package_engines: Some(&pkg),
            filter_engines: vec![EngineConstraintKey::Node, EngineConstraintKey::Npm],
            precision: VersionPrecision::Major,
        };
        let (output, lifecycle) =
            check_engines_with_lifecycle(&input, &cfg(&data, Policy::Lts, true))
                .expect("lifecycle");

        // node ^22 already allowed under Lts → no rewrite. npm ^11 already above floor 10.5.1.
        assert!(lifecycle.bumped_disjuncts.is_empty());
        assert!(lifecycle.dropped_disjuncts.is_empty());
        let bump = lifecycle.npm_bump.expect("bump");
        assert!(!bump.apply);
        // engines_range_to_set may still include normalization differences, but the node
        // computed value should be the unchanged caret form.
        assert_eq!(output.computed_engines[&EngineConstraintKey::Node], "^22");
    }
}
