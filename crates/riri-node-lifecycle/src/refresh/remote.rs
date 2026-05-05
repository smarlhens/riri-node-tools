//! Types describing the upstream JSON feeds.

use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct NodeReleaseEntry {
    pub version: String,
    pub date: NaiveDate,
    pub npm: Option<String>,
    #[serde(default, deserialize_with = "deserialize_lts")]
    pub lts: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleEntry {
    pub start: NaiveDate,
    pub lts: Option<NaiveDate>,
    pub maintenance: Option<NaiveDate>,
    pub end: NaiveDate,
    pub codename: Option<String>,
}

fn deserialize_lts<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum LtsField {
        Bool(#[allow(dead_code)] bool),
        Codename(String),
    }
    Ok(match LtsField::deserialize(d)? {
        LtsField::Bool(_) => None,
        LtsField::Codename(s) => Some(s),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_release_with_codename_lts() {
        let json = r#"{"version":"v22.12.0","date":"2024-12-03","npm":"10.9.0","lts":"Jod"}"#;
        let entry: NodeReleaseEntry = serde_json::from_str(json).expect("parse");
        assert_eq!(entry.lts.as_deref(), Some("Jod"));
        assert_eq!(entry.npm.as_deref(), Some("10.9.0"));
    }

    #[test]
    fn deserializes_release_with_bool_false_lts() {
        let json = r#"{"version":"v23.0.0","date":"2024-10-15","npm":"10.9.0","lts":false}"#;
        let entry: NodeReleaseEntry = serde_json::from_str(json).expect("parse");
        assert!(entry.lts.is_none());
    }

    #[test]
    fn deserializes_release_with_null_npm() {
        let json = r#"{"version":"v0.10.0","date":"2013-03-11","npm":null}"#;
        let entry: NodeReleaseEntry = serde_json::from_str(json).expect("parse");
        assert!(entry.npm.is_none());
    }

    #[test]
    fn deserializes_schedule_entry() {
        let json = r#"{"start":"2023-04-18","lts":"2023-10-24","maintenance":"2024-10-22","end":"2026-04-30","codename":"Iron"}"#;
        let entry: ScheduleEntry = serde_json::from_str(json).expect("parse");
        assert_eq!(entry.codename.as_deref(), Some("Iron"));
        assert_eq!(
            entry.end,
            NaiveDate::from_ymd_opt(2026, 4, 30).expect("end date")
        );
    }
}
