#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! CLI snapshot tests — run the `riri-npd` binary and snapshot its output.

use rstest::rstest;
use std::path::PathBuf;
use std::process::Command;

fn npd_binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_npd"));
    cmd.env("NO_COLOR", "1");
    cmd
}

fn run_in_fixture(fixture: &str, extra_args: &[&str]) -> (String, String, i32) {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(fixture);
    let output = npd_binary()
        .current_dir(&fixture_path)
        .args(extra_args)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[rstest]
fn cli_npd_fixture(
    #[files("../../fixtures/npd-*/package.json")]
    #[exclude("500-deps")]
    #[exclude("workspace")]
    pkg_path: PathBuf,
) {
    let fixture_dir = pkg_path.parent().unwrap();
    let fixture_name = fixture_dir.file_name().unwrap().to_str().unwrap();
    let output = npd_binary()
        .current_dir(fixture_dir)
        .arg("-v")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert!(
        stdout.is_empty(),
        "fixture {fixture_name}: stdout should be empty, got: {stdout}"
    );
    let snapshot = format!("exit: {code}\n---\n{stderr}");
    insta::with_settings!({snapshot_suffix => fixture_name}, {
        insta::assert_snapshot!("cli_npd_fixture", snapshot);
    });
}

#[test]
fn cli_workspace_pin_catalog_includes_catalog_section() {
    let (stdout, _stderr, code) = run_in_fixture(
        "npd-pnpm-v9-workspace",
        &["--pin-catalog", "--json", "--quiet"],
    );
    assert_eq!(code, 1);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(value["members"].is_array());
    let catalog = value["catalog"].as_array().unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["name"], "fake-baz");
}

#[test]
fn cli_workspace_json_schema() {
    let (stdout, _stderr, code) = run_in_fixture("npd-npm-v3-workspace", &["--json", "--quiet"]);
    assert_eq!(code, 1);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let members = value["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    let names: Vec<_> = members
        .iter()
        .map(|m| m["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"@fake/a".to_string()));
    assert!(names.contains(&"@fake/b".to_string()));

    insta::assert_snapshot!("workspace_json_schema", stdout);
}

#[test]
fn cli_workspace_update_rewrites_each_member() {
    let tmp = copy_fixture_to_tmp_workspace("npd-npm-v3-workspace");
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["-u"])
        .output()
        .unwrap();
    assert_eq!(output.status.code().unwrap_or(-1), 1);

    let a = std::fs::read_to_string(tmp.path().join("packages/a/package.json")).unwrap();
    let b = std::fs::read_to_string(tmp.path().join("packages/b/package.json")).unwrap();
    assert!(a.contains("\"fake-foo\": \"1.0.5\""), "a: {a}");
    assert!(b.contains("\"fake-bar\": \"2.3.4\""), "b: {b}");
}

#[test]
fn cli_npm_workspace_lists_per_member_pins() {
    let (stdout, stderr, code) = run_in_fixture("npd-npm-v3-workspace", &["-v"]);
    assert_eq!(code, 1);
    insta::assert_snapshot!("npm_workspace_verbose", stderr);
    let _ = stdout;
}

#[test]
fn cli_unpinned_deps_json_output() {
    let (stdout, _stderr, code) = run_in_fixture("npd-npm-v3-unpinned-deps", &["--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "unpinned_deps_json",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

fn copy_fixture_to_tmp(fixture: &str) -> tempfile::TempDir {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(fixture);
    let tmp = tempfile::TempDir::new().unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let to = tmp.path().join(entry.file_name());
        std::fs::copy(entry.path(), to).unwrap();
    }
    tmp
}

fn copy_fixture_to_tmp_workspace(name: &str) -> tempfile::TempDir {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let dst = tempfile::TempDir::new().unwrap();
    copy_tree(&src, dst.path());
    dst
}

fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&from, &to);
        } else {
            std::fs::copy(&from, &to).unwrap();
        }
    }
}

#[test]
fn cli_update_writes_pinned_versions() {
    let tmp = copy_fixture_to_tmp("npd-npm-v3-unpinned-deps");
    let output = npd_binary()
        .current_dir(tmp.path())
        .arg("-u")
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    let written = std::fs::read_to_string(tmp.path().join("package.json")).unwrap();
    assert!(written.contains("\"foo\": \"4.17.21\""), "{written}");
    assert!(written.contains("\"bar\": \"18.2.0\""), "{written}");
    assert!(written.contains("\"baz\": \"1.6.0\""), "{written}");
    assert!(!written.contains("^4.17.21"), "{written}");
}

#[test]
fn cli_no_lockfile_returns_two() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2);
}

#[test]
fn cli_pnpm_unpinned_deps_strips_peer_suffix() {
    let (stdout, _stderr, code) = run_in_fixture("npd-pnpm-v9-unpinned-deps", &["--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let pins = json.get("pins").and_then(|v| v.as_array()).unwrap();
    let baz = pins.iter().find(|p| p["name"] == "baz").unwrap();
    // peer suffix `(qux@20.0.0)` must be stripped.
    assert_eq!(baz["to"], "1.6.0", "json: {json}");
}

#[test]
fn cli_yarn_resolves_from_lockfile_without_node_modules() {
    // npd parses yarn.lock directly (like the legacy JS), so it resolves with
    // no node_modules present — works under PnP / before install.
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        b"{\"name\":\"x\",\"dependencies\":{\"foo\":\"^4.17.21\"}}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("yarn.lock"),
        b"# yarn lockfile v1\n\nfoo@^4.17.21:\n  version \"4.17.21\"\n",
    )
    .unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .env("NO_COLOR", "1")
        .arg("--json")
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(
        code,
        1,
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    let pins = json["pins"].as_array().unwrap();
    assert_eq!(pins.len(), 1, "json: {json}");
    assert_eq!(pins[0]["name"], "foo");
    assert_eq!(pins[0]["from"], "^4.17.21");
    assert_eq!(pins[0]["to"], "4.17.21");
}

#[test]
fn cli_sort_writes_when_all_pinned() {
    let tmp = copy_fixture_to_tmp("npd-npm-v3-already-pinned");
    let unsorted = "{\n  \"dependencies\": {\n    \"foo\": \"4.17.21\"\n  },\n  \"private\": true,\n  \"name\": \"fake\"\n}\n";
    std::fs::write(tmp.path().join("package.json"), unsorted).unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["--sort"])
        .output()
        .unwrap();
    assert_eq!(output.status.code().unwrap_or(-1), 0);
    let written = std::fs::read_to_string(tmp.path().join("package.json")).unwrap();
    let name_pos = written.find("\"name\"").unwrap();
    let deps_pos = written.find("\"dependencies\"").unwrap();
    assert!(
        name_pos < deps_pos,
        "sort-package-json should place name before dependencies: {written}"
    );
}

#[test]
fn cli_enable_save_exact_creates_npmrc() {
    let tmp = copy_fixture_to_tmp("npd-npm-v3-already-pinned");
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["--enable-save-exact"])
        .output()
        .unwrap();
    assert_eq!(output.status.code().unwrap_or(-1), 0);
    let npmrc = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
    assert_eq!(npmrc, "save-exact=true\n");
}

#[test]
fn cli_enable_save_exact_skips_when_already_set() {
    let tmp = copy_fixture_to_tmp("npd-npm-v3-already-pinned");
    std::fs::write(tmp.path().join(".npmrc"), "save-exact=true\n").unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["--enable-save-exact"])
        .output()
        .unwrap();
    assert_eq!(output.status.code().unwrap_or(-1), 0);
    let npmrc = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
    assert_eq!(npmrc, "save-exact=true\n");
}

#[test]
fn cli_pin_catalog_reports_default_and_named() {
    let (stdout, _stderr, code) =
        run_in_fixture("npd-pnpm-v9-catalog", &["--pin-catalog", "--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "pin_catalog_json",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

fn copy_fixture_dir_to_tmp(fixture: &str) -> tempfile::TempDir {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(fixture);
    let tmp = tempfile::TempDir::new().unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let to = tmp.path().join(entry.file_name());
        std::fs::copy(entry.path(), to).unwrap();
    }
    tmp
}

#[test]
fn cli_pin_catalog_update_rewrites_workspace_yaml() {
    let tmp = copy_fixture_dir_to_tmp("npd-pnpm-v9-catalog");
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["-u", "--pin-catalog"])
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);

    let pkg = std::fs::read_to_string(tmp.path().join("package.json")).unwrap();
    assert!(pkg.contains("\"foo\": \"1.0.5\""), "{pkg}");
    // catalog refs must stay intact in package.json.
    assert!(pkg.contains("\"fizz\": \"catalog:\""), "{pkg}");
    assert!(pkg.contains("\"quux\": \"catalog:set1\""), "{pkg}");

    let yaml = std::fs::read_to_string(tmp.path().join("pnpm-workspace.yaml")).unwrap();
    assert!(yaml.contains("fizz: 18.2.0"), "{yaml}");
    assert!(yaml.contains("quux: 3.4.21"), "{yaml}");
    // already-pinned entry must be left untouched.
    assert!(yaml.contains("buzz: 4.17.21"), "{yaml}");
    // and the unpinned range must be gone.
    assert!(!yaml.contains("^18.0.0"), "{yaml}");
    assert!(!yaml.contains("^3.4.0"), "{yaml}");
}

#[test]
fn cli_pin_catalog_without_update_does_not_touch_yaml() {
    let tmp = copy_fixture_dir_to_tmp("npd-pnpm-v9-catalog");
    let before = std::fs::read_to_string(tmp.path().join("pnpm-workspace.yaml")).unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["--pin-catalog"])
        .output()
        .unwrap();
    assert_eq!(output.status.code().unwrap_or(-1), 1);
    let after = std::fs::read_to_string(tmp.path().join("pnpm-workspace.yaml")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn cli_pin_catalog_on_non_pnpm_project_warns_and_ignores() {
    let tmp = copy_fixture_to_tmp("npd-npm-v3-unpinned-deps");
    let output = npd_binary()
        .current_dir(tmp.path())
        .args(["--pin-catalog", "--json"])
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("pnpm-only"), "stderr: {stderr}");
}

#[test]
fn cli_help_lists_core_flags() {
    let output = npd_binary().arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--update"), "should show --update");
    assert!(stdout.contains("--json"), "should show --json");
    assert!(
        stdout.contains("--enable-save-exact"),
        "should show --enable-save-exact"
    );
}
