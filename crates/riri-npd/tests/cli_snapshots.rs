#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! CLI snapshot tests — run the `riri-npd` binary and snapshot its output.

use rstest::rstest;
use std::path::PathBuf;
use std::process::Command;

fn npd_binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_riri-npd"));
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
fn cli_npd_fixture(#[files("../../fixtures/npd-*/package.json")] pkg_path: PathBuf) {
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
fn cli_yarn_no_node_modules_errors() {
    // yarn.lock without node_modules → scan error → exit 2.
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("package.json"), b"{\"name\":\"x\"}\n").unwrap();
    std::fs::write(tmp.path().join("yarn.lock"), b"# yarn lockfile v1\n").unwrap();
    let output = npd_binary()
        .current_dir(tmp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("node_modules"), "stderr: {stderr}");
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
