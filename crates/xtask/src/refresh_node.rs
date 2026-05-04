//! Refresh the bundled Node.js lifecycle data.

use crate::remote::{NodeReleaseEntry, ScheduleEntry};
use anyhow::Context;
use chrono::{NaiveDate, Utc};
use riri_node_lifecycle::{MajorInfo, ReleaseEntry, Status};
use semver::Version;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Output path. Defaults to the bundled file in `riri-node-lifecycle`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Read pre-fetched JSON files from this directory instead of the network.
    /// Expected files: `index.json`, `schedule.json`.
    #[arg(long)]
    pub offline_from: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct AggregatedData {
    pub schema_version: u32,
    pub fetched_at: chrono::DateTime<Utc>,
    pub majors: BTreeMap<u32, MajorInfo>,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (index_raw, schedule_raw) = if let Some(dir) = &args.offline_from {
        (
            std::fs::read_to_string(dir.join("index.json"))
                .with_context(|| format!("read {}/index.json", dir.display()))?,
            std::fs::read_to_string(dir.join("schedule.json"))
                .with_context(|| format!("read {}/schedule.json", dir.display()))?,
        )
    } else {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build()
            .into();
        let index_raw = agent
            .get("https://nodejs.org/dist/index.json")
            .call()
            .context("fetch index.json")?
            .body_mut()
            .read_to_string()
            .context("read index.json body")?;
        let schedule_raw = agent
            .get("https://raw.githubusercontent.com/nodejs/Release/main/schedule.json")
            .call()
            .context("fetch schedule.json")?
            .body_mut()
            .read_to_string()
            .context("read schedule.json body")?;
        (index_raw, schedule_raw)
    };

    let aggregated = aggregate_offline(&index_raw, &schedule_raw)?;
    let output_path = args.out.clone().unwrap_or_else(default_out_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = std::fs::read_to_string(&output_path).ok();
    if !data_changed(&aggregated, existing.as_deref())? {
        println!("no data changes, skipping write {}", output_path.display());
        return Ok(());
    }
    let json = serde_json::to_string_pretty(&aggregated)? + "\n";
    std::fs::write(&output_path, json)?;
    println!("wrote {}", output_path.display());
    Ok(())
}

/// Returns `true` when the new aggregate differs from the on-disk file in any
/// way other than the volatile `fetched_at` timestamp. A missing or unparseable
/// existing file always counts as changed.
pub fn data_changed(new: &AggregatedData, existing: Option<&str>) -> anyhow::Result<bool> {
    let Some(existing) = existing else {
        return Ok(true);
    };
    let Ok(mut existing_value) = serde_json::from_str::<serde_json::Value>(existing) else {
        return Ok(true);
    };
    let mut new_value = serde_json::to_value(new)?;
    if let Some(obj) = existing_value.as_object_mut() {
        obj.remove("fetched_at");
    }
    if let Some(obj) = new_value.as_object_mut() {
        obj.remove("fetched_at");
    }
    Ok(existing_value != new_value)
}

fn default_out_path() -> PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("xtask manifest under workspace")
        .join("crates/riri-node-lifecycle/data/node-versions.json")
}

pub fn aggregate_offline(index_raw: &str, schedule_raw: &str) -> anyhow::Result<AggregatedData> {
    let releases: Vec<NodeReleaseEntry> = serde_json::from_str(index_raw)?;
    let schedule: BTreeMap<String, ScheduleEntry> = serde_json::from_str(schedule_raw)?;

    let mut by_major: BTreeMap<u32, Vec<(Version, Option<String>, NaiveDate)>> = BTreeMap::new();
    for release in releases {
        let v = release
            .version
            .strip_prefix('v')
            .unwrap_or(&release.version);
        let parsed = Version::parse(v).with_context(|| format!("parse version {v}"))?;
        let major = u32::try_from(parsed.major).context("major overflow")?;
        by_major
            .entry(major)
            .or_default()
            .push((parsed, release.npm, release.date));
    }

    let mut majors = BTreeMap::new();
    for (major, mut versions) in by_major {
        versions.sort_by(|a, b| a.0.cmp(&b.0));

        let release_entries: Vec<ReleaseEntry> = versions
            .iter()
            .filter_map(|(ver, npm, date)| {
                let npm = Version::parse(npm.as_deref()?).ok()?;
                Some(ReleaseEntry {
                    version: ver.clone(),
                    npm,
                    date: *date,
                })
            })
            .collect();

        let schedule_key = format!("v{major}");
        let Some(entry) = schedule.get(&schedule_key) else {
            eprintln!("skipping major {major}: missing from schedule.json");
            continue;
        };

        let Some(lowest) = release_entries.first() else {
            eprintln!("skipping major {major}: no releases with parseable npm");
            continue;
        };
        let highest = release_entries
            .last()
            .expect("non-empty after first() check");
        let npm_min = release_entries
            .iter()
            .map(|r| &r.npm)
            .min()
            .expect("non-empty after first() check")
            .clone();

        let info = MajorInfo {
            major,
            status: derive_status_from_schedule(entry, Utc::now().date_naive()),
            lts_codename: entry.codename.clone(),
            start: entry.start,
            lts_start: entry.lts,
            maintenance_start: entry.maintenance,
            end: entry.end,
            lowest: lowest.version.clone(),
            highest: highest.version.clone(),
            npm_at_lowest: lowest.npm.clone(),
            npm_at_highest: highest.npm.clone(),
            npm_min_in_major: npm_min,
            releases: release_entries,
        };
        majors.insert(major, info);
    }

    Ok(AggregatedData {
        schema_version: 1,
        fetched_at: Utc::now(),
        majors,
    })
}

fn derive_status_from_schedule(s: &ScheduleEntry, today: NaiveDate) -> Status {
    if today < s.start {
        Status::Pending
    } else if today >= s.end {
        Status::EndOfLife
    } else if s.maintenance.is_some_and(|d| today >= d) {
        Status::Maintenance
    } else if s.lts.is_some_and(|d| today >= d) {
        Status::Active
    } else {
        Status::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample_aggregate() -> AggregatedData {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data");
        let index_raw = std::fs::read_to_string(fixtures.join("index.sample.json"))
            .expect("read index fixture");
        let schedule_raw = std::fs::read_to_string(fixtures.join("schedule.sample.json"))
            .expect("read schedule fixture");
        aggregate_offline(&index_raw, &schedule_raw).expect("aggregate")
    }

    #[test]
    fn data_changed_true_when_no_existing_file() {
        let new = sample_aggregate();
        assert!(data_changed(&new, None).expect("compare"));
    }

    #[test]
    fn data_changed_true_when_existing_is_unparseable() {
        let new = sample_aggregate();
        assert!(data_changed(&new, Some("not json")).expect("compare"));
    }

    #[test]
    fn data_changed_false_when_only_fetched_at_differs() {
        let new = sample_aggregate();
        let mut value = serde_json::to_value(&new).expect("serialize");
        value.as_object_mut().expect("object").insert(
            "fetched_at".into(),
            serde_json::json!("2000-01-01T00:00:00Z"),
        );
        let existing = serde_json::to_string(&value).expect("string");
        assert!(!data_changed(&new, Some(&existing)).expect("compare"));
    }

    #[test]
    fn data_changed_true_when_majors_differ() {
        let new = sample_aggregate();
        let mut value = serde_json::to_value(&new).expect("serialize");
        value
            .get_mut("majors")
            .and_then(|m| m.as_object_mut())
            .expect("majors")
            .clear();
        let existing = serde_json::to_string(&value).expect("string");
        assert!(data_changed(&new, Some(&existing)).expect("compare"));
    }

    #[test]
    fn data_changed_true_when_status_differs() {
        let new = sample_aggregate();
        let mut value = serde_json::to_value(&new).expect("serialize");
        let majors = value
            .get_mut("majors")
            .and_then(|m| m.as_object_mut())
            .expect("majors");
        let first = majors.values_mut().next().expect("at least one major");
        first
            .as_object_mut()
            .expect("major object")
            .insert("status".into(), serde_json::json!("pending"));
        let existing = serde_json::to_string(&value).expect("string");
        assert!(data_changed(&new, Some(&existing)).expect("compare"));
    }
}
