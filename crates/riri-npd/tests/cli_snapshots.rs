#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! CLI snapshot tests — run the `riri-npd` binary and snapshot its output.

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

#[test]
fn cli_unpinned_deps_lists_pin_table() {
    let (stdout, stderr, code) = run_in_fixture("npd-npm-v3-unpinned-deps", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("unpinned_deps_stderr", stderr);
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

#[test]
fn cli_already_pinned_returns_zero() {
    let (stdout, stderr, code) = run_in_fixture("npd-npm-v3-already-pinned", &["-v"]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.contains("already pinned"), "stderr: {stderr}");
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
    assert!(written.contains("\"lodash\": \"4.17.21\""), "{written}");
    assert!(written.contains("\"react\": \"18.2.0\""), "{written}");
    assert!(written.contains("\"vitest\": \"1.6.0\""), "{written}");
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
fn cli_help_lists_core_flags() {
    let output = npd_binary().arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--update"), "should show --update");
    assert!(stdout.contains("--json"), "should show --json");
}
