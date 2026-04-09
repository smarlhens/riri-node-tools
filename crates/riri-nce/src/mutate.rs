//! Mutation helpers for applying engine constraint updates.

use crate::EngineRangeToSet;
use riri_common::PackageJsonFile;

/// Apply computed engine updates to a `PackageJsonFile`'s raw JSON value.
///
/// For each engine change, sets `raw["engines"][key] = range_to_set`.
/// Creates the `"engines"` object if it doesn't exist.
pub fn apply_engines_update(pkg: &mut PackageJsonFile, changes: &[EngineRangeToSet]) {
    if changes.is_empty() {
        return;
    }

    let engines = pkg.raw.as_object_mut().and_then(|obj| {
        if !obj.contains_key("engines") {
            obj.insert(
                "engines".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        obj.get_mut("engines")
            .and_then(serde_json::Value::as_object_mut)
    });

    if let Some(engines_obj) = engines {
        for change in changes {
            engines_obj.insert(
                change.engine.to_string(),
                serde_json::Value::String(change.range_to_set.clone()),
            );
        }
    }
}

/// Apply computed engine updates to the lockfile's root entry.
///
/// For npm v2/v3, updates `packages[""]["engines"][key] = range_to_set`.
pub fn apply_engines_to_lockfile(
    lockfile_raw: &mut serde_json::Value,
    changes: &[EngineRangeToSet],
) {
    if changes.is_empty() {
        return;
    }

    let root_entry = lockfile_raw
        .as_object_mut()
        .and_then(|obj| obj.get_mut("packages"))
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|pkgs| pkgs.get_mut(""));

    let Some(root) = root_entry else {
        return;
    };

    let engines = root.as_object_mut().and_then(|obj| {
        if !obj.contains_key("engines") {
            obj.insert(
                "engines".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        obj.get_mut("engines")
            .and_then(serde_json::Value::as_object_mut)
    });

    if let Some(engines_obj) = engines {
        for change in changes {
            engines_obj.insert(
                change.engine.to_string(),
                serde_json::Value::String(change.range_to_set.clone()),
            );
        }
    }
}
