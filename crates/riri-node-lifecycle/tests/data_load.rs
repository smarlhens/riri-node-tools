#![allow(clippy::tests_outside_test_module)]

use chrono::NaiveDate;
use riri_node_lifecycle::LifecycleData;
use tempfile::TempDir;

#[test]
fn bundled_data_parses() {
    let data = LifecycleData::bundled();
    assert!(data.majors.len() >= 2);
    assert!(data.majors.contains_key(&20));
}

#[test]
fn bundled_data_includes_active_lts() {
    let data = LifecycleData::bundled();
    let lts_count = data
        .majors
        .values()
        .filter(|m| m.lts_start.is_some())
        .count();
    assert!(lts_count > 0, "expected at least one LTS major");
}

#[test]
fn bundled_data_releases_are_sorted_ascending() {
    let data = LifecycleData::bundled();
    for (major, info) in &data.majors {
        for pair in info.releases.windows(2) {
            assert!(
                pair[0].version < pair[1].version,
                "major {major} releases not ascending: {} >= {}",
                pair[0].version,
                pair[1].version
            );
        }
    }
}

#[test]
fn cache_override_replaces_bundled_per_major() {
    let dir = TempDir::new().expect("tempdir");
    let cache_path = dir.path().join("node-versions.json");
    std::fs::write(
        &cache_path,
        r#"{
            "schema_version": 1,
            "fetched_at": "2027-01-01T00:00:00Z",
            "majors": {
                "20": {
                    "major": 20,
                    "status": "end-of-life",
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
        }"#,
    )
    .expect("write cache");
    let today = NaiveDate::from_ymd_opt(2027, 1, 1).expect("date");
    let merged = LifecycleData::load_with_cache_override(dir.path(), today).expect("merge");
    assert!(merged.majors.contains_key(&22));
    assert_eq!(
        merged.majors[&20].status,
        riri_node_lifecycle::Status::EndOfLife
    );
}

#[test]
fn cache_override_falls_back_to_bundled_on_missing_file() {
    let dir = TempDir::new().expect("tempdir");
    let today = NaiveDate::from_ymd_opt(2026, 4, 30).expect("date");
    let merged = LifecycleData::load_with_cache_override(dir.path(), today).expect("merge");
    assert!(merged.majors.contains_key(&20));
    assert!(merged.majors.contains_key(&22));
}

#[test]
fn cache_override_falls_back_on_malformed_cache() {
    let dir = TempDir::new().expect("tempdir");
    let cache_path = dir.path().join("node-versions.json");
    std::fs::write(&cache_path, "not json").expect("write cache");
    let today = NaiveDate::from_ymd_opt(2026, 4, 30).expect("date");
    let merged = LifecycleData::load_with_cache_override(dir.path(), today).expect("merge");
    assert!(merged.majors.contains_key(&20));
}
