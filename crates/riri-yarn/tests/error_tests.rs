#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]

use riri_common::LockfileEngines;
use riri_yarn::{YarnProject, YarnScanError};

#[test]
fn node_modules_not_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    let err = YarnProject::scan(tmp.path()).unwrap_err();
    assert!(
        matches!(err, YarnScanError::NodeModulesNotFound(_)),
        "expected NodeModulesNotFound, got: {err}"
    );
}

#[test]
fn empty_node_modules() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
    let project = YarnProject::scan(tmp.path()).unwrap();
    assert_eq!(project.engines_iter().count(), 0);
}

#[test]
fn package_without_engines() {
    let tmp = tempfile::TempDir::new().unwrap();
    let nm = tmp.path().join("node_modules").join("some-pkg");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(
        nm.join("package.json"),
        r#"{"name": "some-pkg", "version": "1.0.0"}"#,
    )
    .unwrap();
    let project = YarnProject::scan(tmp.path()).unwrap();
    assert_eq!(project.engines_iter().count(), 0);
}
