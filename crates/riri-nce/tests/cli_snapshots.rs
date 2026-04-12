#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! CLI snapshot tests — run the `riri-nce` binary and snapshot its output.

use std::process::Command;

fn nce_binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nce"));
    // Disable color for deterministic snapshots
    cmd.env("NO_COLOR", "1");
    cmd
}

fn run_in_fixture(fixture: &str, extra_args: &[&str]) -> (String, String, i32) {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(fixture);

    let output = nce_binary()
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
fn cli_npm_or_ranges_node_only_verbose() {
    let (stdout, stderr, code) = run_in_fixture("npm-v3-or-ranges-node-only", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("npm_or_ranges_node_only_verbose_stderr", stderr);
}

#[test]
fn cli_npm_or_ranges_node_npm_yarn_verbose() {
    let (stdout, stderr, code) = run_in_fixture("npm-v3-or-ranges-node-npm-yarn", &["-v"]);
    assert_eq!(code, 1);
    insta::assert_snapshot!("npm_or_ranges_node_npm_yarn_verbose_stderr", stderr);
    assert!(stdout.is_empty());
}

#[test]
fn cli_npm_up_to_date_verbose() {
    let (stdout, stderr, code) = run_in_fixture("npm-v1-deps-field", &["-v"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!("npm_up_to_date_verbose_stderr", stderr);
    assert!(stdout.is_empty());
}

#[test]
fn cli_npm_json_output() {
    let (stdout, _stderr, code) = run_in_fixture("npm-v3-or-ranges-node-only", &["--json"]);
    assert_eq!(code, 1);
    // Parse to normalize key order, then snapshot
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "npm_json_output",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

#[test]
fn cli_npm_engine_filter_verbose() {
    let (stdout, stderr, code) =
        run_in_fixture("npm-v3-or-ranges-node-npm-yarn", &["-v", "-e", "node"]);
    assert_eq!(code, 1);
    insta::assert_snapshot!("npm_engine_filter_verbose_stderr", stderr);
    assert!(stdout.is_empty());
}

#[test]
fn cli_npm_quiet_mode() {
    let (stdout, stderr, code) = run_in_fixture("npm-v3-or-ranges-node-only", &["-q"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
}

#[test]
fn cli_no_lockfile() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = nce_binary()
        .current_dir(tmp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no lockfile found"), "stderr: {stderr}");
}

#[test]
fn cli_help() {
    let output = nce_binary().arg("--help").output().unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--update"), "should show --update flag");
    assert!(stdout.contains("--engines"), "should show --engines flag");
    assert!(stdout.contains("--json"), "should show --json flag");
}

#[test]
fn cli_pnpm_or_ranges_node_only_verbose() {
    let (stdout, stderr, code) = run_in_fixture("pnpm-v9-or-ranges-node-only", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("pnpm_or_ranges_node_only_verbose_stderr", stderr);
}

#[test]
fn cli_pnpm_or_ranges_node_npm_yarn_verbose() {
    let (stdout, stderr, code) = run_in_fixture("pnpm-v9-or-ranges-node-npm-yarn", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("pnpm_or_ranges_node_npm_yarn_verbose_stderr", stderr);
}

#[test]
fn cli_pnpm_json_output() {
    let (stdout, _stderr, code) = run_in_fixture("pnpm-v9-or-ranges-node-only", &["--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "pnpm_json_output",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

#[test]
fn cli_yarn_or_ranges_node_only_verbose() {
    let (stdout, stderr, code) = run_in_fixture("yarn-v1-or-ranges-node-only", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("yarn_or_ranges_node_only_verbose_stderr", stderr);
}

#[test]
fn cli_yarn_or_ranges_node_npm_yarn_verbose() {
    let (stdout, stderr, code) = run_in_fixture("yarn-v4-or-ranges-node-npm-yarn", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("yarn_or_ranges_node_npm_yarn_verbose_stderr", stderr);
}

#[test]
fn cli_yarn_json_output() {
    let (stdout, _stderr, code) = run_in_fixture("yarn-v1-or-ranges-node-only", &["--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "yarn_json_output",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

#[test]
fn cli_yarn_up_to_date_verbose() {
    let (stdout, stderr, code) = run_in_fixture("yarn-v1-up-to-date", &["-v"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!("yarn_up_to_date_verbose_stderr", stderr);
    assert!(stdout.is_empty());
}

#[test]
fn cli_yarn_no_node_modules() {
    let (stdout, stderr, code) = run_in_fixture("yarn-v1-no-node-modules", &["-v"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("node_modules"),
        "error should mention node_modules: {stderr}"
    );
}
