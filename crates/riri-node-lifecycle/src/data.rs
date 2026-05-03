//! Aggregate lifecycle data + lookup API.

use crate::major::MajorInfo;
use crate::schema::{SCHEMA_VERSION, SchemaError};
use crate::{Policy, Status};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LookupError {
    #[error("invalid lifecycle data JSON")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleData {
    pub schema_version: u32,
    pub fetched_at: DateTime<Utc>,
    pub majors: BTreeMap<u32, MajorInfo>,
}

impl LifecycleData {
    /// Parses lifecycle data from a JSON string and validates the schema version.
    ///
    /// # Errors
    ///
    /// Returns [`LookupError::Json`] when the input is not valid JSON or does not match
    /// the [`LifecycleData`] shape, and [`LookupError::Schema`] when `schema_version`
    /// exceeds [`SCHEMA_VERSION`].
    pub fn parse(s: &str) -> Result<Self, LookupError> {
        let parsed: Self = serde_json::from_str(s)?;
        if parsed.schema_version != SCHEMA_VERSION {
            return Err(SchemaError::UnsupportedVersion {
                found: parsed.schema_version,
            }
            .into());
        }
        Ok(parsed)
    }

    /// Re-resolves [`Status`] for each major using `today`.
    ///
    /// Order of checks: `today < start` → Pending; `today >= end` → `EndOfLife`;
    /// `today >= maintenance_start` → Maintenance; `today >= lts_start` → Active;
    /// otherwise → Current.
    pub fn resolve_statuses(&mut self, today: NaiveDate) {
        for major in self.majors.values_mut() {
            major.status = derive_status(major, today);
        }
    }

    /// Returns sorted ascending list of majors whose current `status` is in
    /// the policy's allowed set.
    #[must_use]
    pub fn allowed_majors(&self, policy: Policy) -> Vec<u32> {
        let allowed = policy.allowed_statuses();
        self.majors
            .iter()
            .filter(|(_, m)| allowed.contains(&m.status))
            .map(|(k, _)| *k)
            .collect()
    }

    /// Returns the conservative npm floor for any release of the same major.
    #[must_use]
    pub fn npm_for_node_version(&self, v: &semver::Version) -> Option<&semver::Version> {
        let major = u32::try_from(v.major).ok()?;
        self.majors.get(&major).map(|m| &m.npm_min_in_major)
    }

    /// Returns the npm shipped with the highest known release `r` in the same major
    /// where `r.version <= v`.
    ///
    /// Returns `None` when the major is unknown or `v` is below the lowest known
    /// release in that major.
    #[must_use]
    pub fn npm_at_node_version(&self, v: &semver::Version) -> Option<&semver::Version> {
        let major = u32::try_from(v.major).ok()?;
        let info = self.majors.get(&major)?;
        info.releases
            .iter()
            .rev()
            .find(|r| r.version <= *v)
            .map(|r| &r.npm)
    }

    /// Loads the bundled snapshot, then merges any per-major entries from
    /// `<cache_dir>/node-versions.json` on top, then re-resolves status against
    /// `today`.
    ///
    /// # Errors
    ///
    /// Reserved for future expansion — currently always returns `Ok`. A missing or
    /// malformed cache file is treated as "no override" and falls back to the bundled
    /// snapshot.
    pub fn load_with_cache_override(
        cache_dir: &std::path::Path,
        today: NaiveDate,
    ) -> Result<Self, LookupError> {
        let mut merged = Self::bundled().clone();
        if let Some(cache) = crate::cache::try_load(cache_dir) {
            if cache.fetched_at > merged.fetched_at {
                merged.fetched_at = cache.fetched_at;
            }
            for (major, info) in cache.majors {
                merged.majors.insert(major, info);
            }
        }
        merged.resolve_statuses(today);
        Ok(merged)
    }

    /// Returns the bundled snapshot, parsed once and cached for the process lifetime.
    ///
    /// # Panics
    ///
    /// Panics only if the bundled data file fails to parse, which would indicate
    /// a build-time mistake (the file is `include_str!`'d at compile time).
    #[must_use]
    pub fn bundled() -> &'static Self {
        use std::sync::OnceLock;
        static CELL: OnceLock<LifecycleData> = OnceLock::new();
        CELL.get_or_init(|| {
            const RAW: &str = include_str!("../data/node-versions.json");
            Self::parse(RAW).expect("bundled data must parse")
        })
    }
}

fn derive_status(m: &MajorInfo, today: NaiveDate) -> Status {
    if today < m.start {
        Status::Pending
    } else if today >= m.end {
        Status::EndOfLife
    } else if m.maintenance_start.is_some_and(|d| today >= d) {
        Status::Maintenance
    } else if m.lts_start.is_some_and(|d| today >= d) {
        Status::Active
    } else {
        Status::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Status;

    fn sample_json() -> &'static str {
        r#"{
            "schema_version": 1,
            "fetched_at": "2026-04-29T00:00:00Z",
            "majors": {
                "20": {
                    "major": 20,
                    "status": "maintenance",
                    "lts_codename": "Iron",
                    "start": "2023-04-18",
                    "lts_start": "2023-10-24",
                    "maintenance_start": "2024-10-22",
                    "end": "2026-04-30",
                    "lowest": "20.0.0",
                    "highest": "20.19.5",
                    "npm_at_lowest": "9.6.4",
                    "npm_at_highest": "10.8.2",
                    "npm_min_in_major": "9.6.4",
                    "releases": [
                        {"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"}
                    ]
                }
            }
        }"#
    }

    #[test]
    fn parses_lifecycle_data() {
        let data = LifecycleData::parse(sample_json()).expect("parse");
        assert_eq!(data.schema_version, 1);
        assert_eq!(data.majors.len(), 1);
        assert_eq!(data.majors[&20].status, Status::Maintenance);
    }

    #[test]
    fn rejects_future_schema_version() {
        let json = sample_json().replace("\"schema_version\": 1", "\"schema_version\": 999");
        let err = LifecycleData::parse(&json).expect_err("future schema must fail");
        assert!(matches!(err, LookupError::Schema(_)));
    }

    #[test]
    fn rejects_zero_schema_version() {
        let json = sample_json().replace("\"schema_version\": 1", "\"schema_version\": 0");
        let err = LifecycleData::parse(&json).expect_err("zero schema must fail");
        assert!(matches!(err, LookupError::Schema(_)));
    }

    fn three_majors_json() -> &'static str {
        r#"{
            "schema_version": 1,
            "fetched_at": "2026-04-29T00:00:00Z",
            "majors": {
                "20": {
                    "major": 20,
                    "status": "active",
                    "lts_codename": "Iron",
                    "start": "2023-04-18",
                    "lts_start": "2023-10-24",
                    "maintenance_start": "2024-10-22",
                    "end": "2026-04-30",
                    "lowest": "20.0.0",
                    "highest": "20.19.5",
                    "npm_at_lowest": "9.6.4",
                    "npm_at_highest": "10.8.2",
                    "npm_min_in_major": "9.6.4",
                    "releases": [
                        {"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"}
                    ]
                },
                "26": {
                    "major": 26,
                    "status": "pending",
                    "lts_codename": null,
                    "start": "2026-10-20",
                    "lts_start": null,
                    "maintenance_start": null,
                    "end": "2029-04-30",
                    "lowest": "26.0.0",
                    "highest": "26.0.0",
                    "npm_at_lowest": "11.0.0",
                    "npm_at_highest": "11.0.0",
                    "npm_min_in_major": "11.0.0",
                    "releases": [
                        {"version": "26.0.0", "npm": "11.0.0", "date": "2026-10-20"}
                    ]
                }
            }
        }"#
    }

    #[test]
    fn resolve_statuses_treats_pre_start_as_pending() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2026, 4, 29).expect("date"));
        assert_eq!(data.majors[&26].status, Status::Pending);
    }

    #[test]
    fn resolve_statuses_treats_post_end_as_eol() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2027, 1, 1).expect("date"));
        assert_eq!(data.majors[&20].status, Status::EndOfLife);
    }

    #[test]
    fn resolve_statuses_promotes_to_maintenance() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2024, 11, 1).expect("date"));
        assert_eq!(data.majors[&20].status, Status::Maintenance);
    }

    #[test]
    fn resolve_statuses_keeps_active_before_maintenance_start() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2024, 5, 1).expect("date"));
        assert_eq!(data.majors[&20].status, Status::Active);
    }

    #[test]
    fn allowed_majors_under_lts_keeps_active_and_maintenance() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2024, 11, 1).expect("date"));
        let majors = data.allowed_majors(Policy::Lts);
        assert_eq!(majors, vec![20]);
    }

    #[test]
    fn allowed_majors_under_supported_excludes_eol() {
        let mut data = LifecycleData::parse(three_majors_json()).expect("parse");
        data.resolve_statuses(NaiveDate::from_ymd_opt(2027, 1, 1).expect("date"));
        let majors = data.allowed_majors(Policy::Supported);
        assert!(!majors.contains(&20));
        assert!(majors.contains(&26));
    }

    #[test]
    fn npm_for_known_node_version_within_major_returns_npm_min() {
        let data = LifecycleData::parse(three_majors_json()).expect("parse");
        let v = semver::Version::parse("20.5.0").expect("ver");
        let npm = data.npm_for_node_version(&v).expect("npm");
        assert_eq!(npm, &semver::Version::parse("9.6.4").expect("npm ver"));
    }

    #[test]
    fn npm_for_unknown_major_returns_none() {
        let data = LifecycleData::parse(three_majors_json()).expect("parse");
        let v = semver::Version::parse("18.0.0").expect("ver");
        assert!(data.npm_for_node_version(&v).is_none());
    }

    fn json_with_releases() -> &'static str {
        r#"{
            "schema_version": 1,
            "fetched_at": "2026-04-29T00:00:00Z",
            "majors": {
                "20": {
                    "major": 20,
                    "status": "maintenance",
                    "lts_codename": "Iron",
                    "start": "2023-04-18",
                    "lts_start": "2023-10-24",
                    "maintenance_start": "2024-10-22",
                    "end": "2026-04-30",
                    "lowest": "20.0.0",
                    "highest": "20.10.0",
                    "npm_at_lowest": "9.6.4",
                    "npm_at_highest": "10.2.3",
                    "npm_min_in_major": "9.6.4",
                    "releases": [
                        {"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"},
                        {"version": "20.10.0", "npm": "10.2.3", "date": "2023-11-22"}
                    ]
                }
            }
        }"#
    }

    #[test]
    fn npm_at_exact_match() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        let v = semver::Version::parse("20.0.0").expect("v");
        assert_eq!(
            data.npm_at_node_version(&v).expect("npm"),
            &semver::Version::parse("9.6.4").expect("npm v")
        );
    }

    #[test]
    fn npm_at_between_bumps_returns_lower() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        let v = semver::Version::parse("20.5.0").expect("v");
        assert_eq!(
            data.npm_at_node_version(&v).expect("npm"),
            &semver::Version::parse("9.6.4").expect("npm v")
        );
    }

    #[test]
    fn npm_at_above_highest_returns_highest() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        let v = semver::Version::parse("20.999.0").expect("v");
        assert_eq!(
            data.npm_at_node_version(&v).expect("npm"),
            &semver::Version::parse("10.2.3").expect("npm v")
        );
    }

    #[test]
    fn npm_at_below_lowest_returns_none() {
        let json = json_with_releases().replace(
            r#"{"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"}"#,
            r#"{"version": "20.5.0", "npm": "9.6.4", "date": "2023-04-18"}"#,
        );
        let data = LifecycleData::parse(&json).expect("parse");
        let v = semver::Version::parse("20.0.0").expect("v");
        assert!(data.npm_at_node_version(&v).is_none());
    }

    #[test]
    fn npm_at_unknown_major_returns_none() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        let v = semver::Version::parse("18.0.0").expect("v");
        assert!(data.npm_at_node_version(&v).is_none());
    }

    #[test]
    fn npm_at_prerelease_treats_as_below_release() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        // 20.5.0-beta.1 < 20.5.0, but > 20.0.0 — should match 20.0.0's npm
        let v = semver::Version::parse("20.5.0-beta.1").expect("v");
        assert_eq!(
            data.npm_at_node_version(&v).expect("npm"),
            &semver::Version::parse("9.6.4").expect("npm v")
        );
    }

    #[test]
    fn npm_for_major_zero_returns_none() {
        let data = LifecycleData::parse(json_with_releases()).expect("parse");
        let v = semver::Version::parse("0.10.0").expect("v");
        assert!(data.npm_for_node_version(&v).is_none());
        assert!(data.npm_at_node_version(&v).is_none());
    }
}
