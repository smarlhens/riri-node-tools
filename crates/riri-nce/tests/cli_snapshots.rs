#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::unwrap_used)]
//! CLI snapshot tests — run the `riri-nce` binary and snapshot its output.

use std::process::Command;

fn nce_binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_riri-nce"));
    // Disable color for deterministic snapshots
    cmd.env("NO_COLOR", "1");
    cmd
}

fn frozen_data_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/lifecycle-2026-04-29.json")
        .to_string_lossy()
        .into_owned()
}

fn pin_lifecycle(extra_args: &[&str]) -> Vec<String> {
    let frozen = frozen_data_path();
    let mut args: Vec<String> = vec![
        "--today".into(),
        "2026-04-29".into(),
        "--node-data".into(),
        frozen,
    ];
    args.extend(extra_args.iter().map(|s| (*s).to_string()));
    args
}

fn run_in_fixture(fixture: &str, extra_args: &[&str]) -> (String, String, i32) {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(fixture);

    let pinned = pin_lifecycle(extra_args);
    let output = nce_binary()
        .current_dir(&fixture_path)
        .args(&pinned)
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
    let (stdout, stderr, code) = run_in_fixture(
        "npm-v1-deps-field",
        &["-v", "--node-policy=any", "--no-bump-npm"],
    );
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
    let (stdout, stderr, code) = run_in_fixture(
        "yarn-v1-up-to-date",
        &["-v", "--node-policy=any", "--no-bump-npm"],
    );
    assert_eq!(code, 0);
    insta::assert_snapshot!("yarn_up_to_date_verbose_stderr", stderr);
    assert!(stdout.is_empty());
}

#[test]
fn cli_policy_supported_eol_bump() {
    let (stdout, stderr, code) = run_in_fixture("nce-policy-supported-eol-bump", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_supported_eol_bump_stderr", stderr);
}

#[test]
fn cli_policy_lts_eol_bump() {
    let (stdout, stderr, code) =
        run_in_fixture("nce-policy-lts-eol-bump", &["-v", "--node-policy=lts"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_lts_eol_bump_stderr", stderr);
}

#[test]
fn cli_policy_eol_warning_under_any() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-allow-eol-suppresses-warn",
        &["-v", "--node-policy=any", "--no-bump-npm"],
    );
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    assert!(stderr.contains("end-of-life"), "stderr: {stderr}");
    insta::assert_snapshot!("policy_eol_warning_under_any_stderr", stderr);
}

#[test]
fn cli_policy_allow_eol_suppresses_warning() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-allow-eol-suppresses-warn",
        &["-v", "--node-policy=any", "--allow-eol", "--no-bump-npm"],
    );
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(!stderr.contains("warning:"), "stderr: {stderr}");
    insta::assert_snapshot!("policy_allow_eol_suppresses_warning_stderr", stderr);
}

#[test]
fn cli_npm_bump_floor() {
    let (stdout, stderr, code) = run_in_fixture("nce-npm-bump-floor", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("npm_bump_floor_stderr", stderr);
}

#[test]
fn cli_no_bump_npm_flag() {
    let (stdout, stderr, code) = run_in_fixture("nce-no-bump-npm-flag", &["-v", "--no-bump-npm"]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("no_bump_npm_flag_stderr", stderr);
}

#[test]
fn cli_npm_precision_minor() {
    let (stdout, stderr, code) =
        run_in_fixture("nce-npm-precision-minor", &["-v", "--npm-precision=minor"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("npm_precision_minor_stderr", stderr);
}

#[test]
fn cli_policy_stable_keeps_eol_with_warning() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-allow-eol-suppresses-warn",
        &["-v", "--node-policy=stable", "--no-bump-npm"],
    );
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    insta::assert_snapshot!("policy_stable_keeps_eol_with_warning_stderr", stderr);
}

#[test]
fn cli_policy_maintenance_drops_active() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-maintenance-drops-active",
        &["-v", "--node-policy=maintenance", "--no-bump-npm"],
    );
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_maintenance_drops_active_stderr", stderr);
}

#[test]
fn cli_policy_supported_keeps_current_caret() {
    let (stdout, stderr, code) =
        run_in_fixture("nce-policy-supported-keeps-current-caret", &["-v"]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_supported_keeps_current_caret_stderr", stderr);
}

#[test]
fn cli_policy_supported_narrows_compound_bounded() {
    let (stdout, stderr, code) = run_in_fixture("nce-policy-supported-narrows-compound", &["-v"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_supported_narrows_compound_stderr", stderr);
}

#[test]
fn cli_policy_lts_expands_wildcard() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-lts-expands-wildcard",
        &["-v", "--node-policy=lts", "--no-bump-npm"],
    );
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("policy_lts_expands_wildcard_stderr", stderr);
}

#[test]
fn cli_npm_precision_patch_explicit() {
    let (stdout, stderr, code) =
        run_in_fixture("nce-npm-precision-minor", &["-v", "--npm-precision=patch"]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("npm_precision_patch_explicit_stderr", stderr);
}

#[test]
fn cli_bump_npm_overrides_no_bump() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-no-bump-npm-flag",
        &["-v", "--no-bump-npm", "--bump-npm"],
    );
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    insta::assert_snapshot!("bump_npm_overrides_no_bump_stderr", stderr);
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
fn cli_update_writes_lifecycle_rewrite() {
    let tmp = copy_fixture_to_tmp("nce-policy-supported-eol-bump");
    let pinned = pin_lifecycle(&["-u", "--no-bump-npm"]);
    let output = nce_binary()
        .current_dir(tmp.path())
        .args(&pinned)
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    let written = std::fs::read_to_string(tmp.path().join("package.json")).unwrap();
    assert!(written.contains("\">=20.0.0\""), "package.json: {written}");
    assert!(!written.contains("\">=18.0.0\""), "package.json: {written}");
}

#[test]
fn cli_policy_unsatisfiable() {
    let (stdout, stderr, code) = run_in_fixture(
        "nce-policy-unsatisfiable",
        &["-v", "--node-policy=maintenance"],
    );
    assert_eq!(code, 3);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unsatisfiable"), "stderr: {stderr}");
    insta::assert_snapshot!("policy_unsatisfiable_stderr", stderr);
}

#[test]
fn cli_policy_unsatisfiable_json() {
    let (stdout, _stderr, code) = run_in_fixture(
        "nce-policy-unsatisfiable",
        &["--json", "--node-policy=maintenance"],
    );
    assert_eq!(code, 3);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "policy_unsatisfiable_json",
        serde_json::to_string_pretty(&json).unwrap()
    );
}

#[test]
fn cli_supported_eol_bump_json() {
    let (stdout, _stderr, code) = run_in_fixture("nce-policy-supported-eol-bump", &["--json"]);
    assert_eq!(code, 1);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    insta::assert_snapshot!(
        "supported_eol_bump_json",
        serde_json::to_string_pretty(&json).unwrap()
    );
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
