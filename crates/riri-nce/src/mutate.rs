//! Mutation helpers for applying engine constraint updates.

use crate::EngineRangeToSet;

/// Set `pkg_raw["engines"][key] = range_to_set` for each change, creating the
/// `"engines"` object when absent.
pub fn apply_engines_update(pkg_raw: &mut serde_json::Value, changes: &[EngineRangeToSet]) {
    if changes.is_empty() {
        return;
    }

    let Some(obj) = pkg_raw.as_object_mut() else {
        return;
    };
    let Some(engines) = obj
        .entry("engines")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
    else {
        return;
    };

    for change in changes {
        engines.insert(
            change.engine.to_string(),
            serde_json::Value::String(change.range_to_set.clone()),
        );
    }
}

/// Same, on an npm v2/v3 lockfile's root entry (`packages[""]`).
pub fn apply_engines_to_lockfile(
    lockfile_raw: &mut serde_json::Value,
    changes: &[EngineRangeToSet],
) {
    let root_entry = lockfile_raw
        .as_object_mut()
        .and_then(|obj| obj.get_mut("packages"))
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|pkgs| pkgs.get_mut(""));

    if let Some(root) = root_entry {
        apply_engines_update(root, changes);
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_engines_to_lockfile, apply_engines_update};
    use crate::{EngineConstraintKey, EngineRangeToSet};

    fn change(engine: EngineConstraintKey, range_to_set: &str) -> EngineRangeToSet {
        EngineRangeToSet {
            engine,
            range: "*".to_string(),
            range_to_set: range_to_set.to_string(),
        }
    }

    #[test]
    fn creates_engines_object_when_absent() {
        let mut pkg = serde_json::json!({"name": "app"});
        apply_engines_update(&mut pkg, &[change(EngineConstraintKey::Node, ">=20.0.0")]);
        assert_eq!(
            pkg,
            serde_json::json!({"name": "app", "engines": {"node": ">=20.0.0"}})
        );
    }

    #[test]
    fn overwrites_existing_engine_and_keeps_the_others() {
        let mut pkg = serde_json::json!({"engines": {"node": ">=14.0.0", "npm": ">=6.0.0"}});
        apply_engines_update(&mut pkg, &[change(EngineConstraintKey::Node, ">=20.0.0")]);
        assert_eq!(
            pkg,
            serde_json::json!({"engines": {"node": ">=20.0.0", "npm": ">=6.0.0"}})
        );
    }

    #[test]
    fn leaves_the_value_untouched_when_there_is_nothing_to_change() {
        let mut pkg = serde_json::json!({"name": "app"});
        apply_engines_update(&mut pkg, &[]);
        assert_eq!(pkg, serde_json::json!({"name": "app"}));
    }

    #[test]
    fn lockfile_update_targets_the_root_entry() {
        let mut lock = serde_json::json!({"packages": {"": {"name": "app"}, "node_modules/a": {}}});
        apply_engines_to_lockfile(&mut lock, &[change(EngineConstraintKey::Node, ">=20.0.0")]);
        assert_eq!(
            lock,
            serde_json::json!({
                "packages": {
                    "": {"name": "app", "engines": {"node": ">=20.0.0"}},
                    "node_modules/a": {}
                }
            })
        );
    }

    #[test]
    fn lockfile_update_is_a_no_op_without_a_root_entry() {
        let mut lock = serde_json::json!({"packages": {"node_modules/a": {}}});
        apply_engines_to_lockfile(&mut lock, &[change(EngineConstraintKey::Node, ">=20.0.0")]);
        assert_eq!(
            lock,
            serde_json::json!({"packages": {"node_modules/a": {}}})
        );
    }
}
