#![allow(clippy::tests_outside_test_module)]

use std::path::Path;

fn strip_volatile(v: &mut serde_json::Value) {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("fetched_at");
    }
    if let Some(majors) = v.get_mut("majors").and_then(|m| m.as_object_mut()) {
        for entry in majors.values_mut() {
            if let Some(obj) = entry.as_object_mut() {
                obj.remove("status");
            }
        }
    }
}

#[test]
fn aggregate_offline_matches_expected() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data");
    let index_raw =
        std::fs::read_to_string(fixtures.join("index.sample.json")).expect("read index");
    let schedule_raw =
        std::fs::read_to_string(fixtures.join("schedule.sample.json")).expect("read schedule");
    let mut expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures.join("expected.json")).expect("read expected"),
    )
    .expect("parse expected");

    let aggregated =
        xtask::refresh_node::aggregate_offline(&index_raw, &schedule_raw).expect("aggregate");
    let mut value: serde_json::Value =
        serde_json::to_value(&aggregated).expect("serialize aggregated");

    strip_volatile(&mut value);
    strip_volatile(&mut expected);

    assert_eq!(value, expected);
}
