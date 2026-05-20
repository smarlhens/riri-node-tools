#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]

use riri_workspace::detect;
use std::fs;
use tempfile::TempDir;

fn setup(workspaces: &str, members: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        format!(r#"{{"workspaces":{workspaces}}}"#),
    )
    .unwrap();
    fs::write(tmp.path().join("package-lock.json"), "").unwrap();
    for (rel, name) in members {
        let dir = tmp.path().join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), format!(r#"{{"name":"{name}"}}"#)).unwrap();
    }
    tmp
}

#[test]
fn enumerates_simple_glob() {
    let tmp = setup(
        r#"["packages/*"]"#,
        &[("packages/a", "@scope/a"), ("packages/b", "@scope/b")],
    );
    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    let names: Vec<_> = members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["@scope/a", "@scope/b"]);
}

#[test]
fn skips_node_modules() {
    let tmp = setup(
        r#"["packages/*"]"#,
        &[
            ("packages/a", "a"),
            ("node_modules/x/packages/poison", "poison"),
        ],
    );
    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name, "a");
}

#[test]
fn skips_dotdirs() {
    let tmp = setup(
        r#"["packages/*"]"#,
        &[("packages/a", "a"), (".cache/packages/poison", "poison")],
    );
    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    assert_eq!(members.len(), 1);
}

#[test]
fn falls_back_to_relative_path_when_name_missing() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(tmp.path().join("package-lock.json"), "").unwrap();
    let dir = tmp.path().join("packages/unnamed");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("package.json"), "{}").unwrap();

    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    assert_eq!(members[0].name, "packages/unnamed");
}

#[test]
fn dedups_overlapping_globs() {
    let tmp = setup(
        r#"["packages/*","packages/a"]"#,
        &[("packages/a", "a"), ("packages/b", "b")],
    );
    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    assert_eq!(members.len(), 2);
}

#[test]
fn empty_when_no_glob_matches() {
    let tmp = setup(r#"["packages/*"]"#, &[]);
    let project = detect(tmp.path()).unwrap();
    let members = project.members().unwrap();
    assert!(members.is_empty());
}
