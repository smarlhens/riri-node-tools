#![allow(clippy::tests_outside_test_module)]

use riri_ncd::{
    DeprecatedField, DeprecationSource, Packument, PackumentVersion, SourceError,
    check_deprecations,
};
use std::collections::HashMap;

const PKG: &str = r#"{"name": "app", "dependencies": {"typescript": "^5.4.0"}}"#;

const NPM_LOCK: &str = r#"{
    "lockfileVersion": 3,
    "packages": {
        "": {"name": "app", "dependencies": {"typescript": "^5.4.0"}},
        "node_modules/typescript": {"version": "5.4.5"}
    }
}"#;

/// Offline source: `typescript@5.4.5` is deprecated, `5.5.0` is not.
struct StubSource;

impl DeprecationSource for StubSource {
    fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
        if name != "typescript" {
            return Ok(None);
        }
        let mut versions = HashMap::new();
        versions.insert(
            "5.4.5".to_string(),
            PackumentVersion {
                deprecated: Some(DeprecatedField::Message("use 5.5.0".into())),
                ..PackumentVersion::default()
            },
        );
        versions.insert("5.5.0".to_string(), PackumentVersion::default());
        Ok(Some(Packument {
            dist_tags: HashMap::from([("latest".to_string(), "5.5.0".to_string())]),
            versions,
        }))
    }
}

#[test]
fn checking_deprecations_needs_no_crate_beyond_riri_ncd() {
    let report = check_deprecations(PKG, NPM_LOCK, "npm", &StubSource).expect("report");

    assert_eq!(report.deprecated.len(), 1);
    assert_eq!(report.deprecated[0].name, "typescript");
    assert_eq!(report.deprecated[0].version, "5.4.5");
}
