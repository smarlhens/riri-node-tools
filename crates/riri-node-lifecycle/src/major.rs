//! Per-major lifecycle data.

use chrono::NaiveDate;
use semver::Version;
use serde::{Deserialize, Serialize};

/// Lifecycle phase of a Node.js major release.
///
/// Stored on disk via `#[serde(rename_all = "kebab-case")]` (e.g.
/// `Status::EndOfLife` → `"end-of-life"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pending,
    Current,
    Active,
    Maintenance,
    EndOfLife,
}

/// A single Node.js release with its bundled npm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEntry {
    pub version: Version,
    pub npm: Version,
    pub date: NaiveDate,
}

/// Aggregated lifecycle + release metadata for a single Node.js major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajorInfo {
    pub major: u32,
    pub status: Status,
    pub lts_codename: Option<String>,
    pub start: NaiveDate,
    pub lts_start: Option<NaiveDate>,
    pub maintenance_start: Option<NaiveDate>,
    pub end: NaiveDate,
    pub lowest: Version,
    pub highest: Version,
    pub npm_at_lowest: Version,
    pub npm_at_highest: Version,
    pub npm_min_in_major: Version,
    pub releases: Vec<ReleaseEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use semver::Version;

    fn sample_json() -> &'static str {
        r#"{
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
        }"#
    }

    #[test]
    fn major_info_round_trips_through_serde() {
        let parsed: MajorInfo = serde_json::from_str(sample_json()).expect("parse");
        assert_eq!(parsed.major, 20);
        assert_eq!(parsed.status, Status::Maintenance);
        assert_eq!(parsed.lts_codename.as_deref(), Some("Iron"));
        assert_eq!(
            parsed.start,
            NaiveDate::from_ymd_opt(2023, 4, 18).expect("start date")
        );
        assert_eq!(
            parsed.maintenance_start,
            Some(NaiveDate::from_ymd_opt(2024, 10, 22).expect("maintenance date"))
        );
        assert_eq!(
            parsed.end,
            NaiveDate::from_ymd_opt(2026, 4, 30).expect("end date")
        );
        assert_eq!(parsed.lowest, Version::parse("20.0.0").expect("lowest"));
        assert_eq!(
            parsed.npm_at_lowest,
            Version::parse("9.6.4").expect("npm at lowest")
        );
        let re_serialized = serde_json::to_value(&parsed).expect("serialize");
        let original: serde_json::Value =
            serde_json::from_str(sample_json()).expect("parse original");
        assert_eq!(re_serialized, original);
    }

    #[test]
    fn major_info_round_trips_with_releases() {
        let json = r#"{
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
                {"version": "20.0.0", "npm": "9.6.4", "date": "2023-04-18"},
                {"version": "20.10.0", "npm": "10.2.3", "date": "2023-11-22"}
            ]
        }"#;
        let parsed: MajorInfo = serde_json::from_str(json).expect("parse");
        assert_eq!(parsed.releases.len(), 2);
        assert_eq!(
            parsed.releases[0].version,
            Version::parse("20.0.0").expect("v")
        );
        assert_eq!(
            parsed.releases[1].npm,
            Version::parse("10.2.3").expect("npm")
        );
        assert_eq!(
            parsed.releases[0].date,
            NaiveDate::from_ymd_opt(2023, 4, 18).expect("date")
        );
        let re_serialized = serde_json::to_value(&parsed).expect("serialize");
        let original: serde_json::Value = serde_json::from_str(json).expect("parse original");
        assert_eq!(re_serialized, original);
    }
}
