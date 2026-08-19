//! `msrv.yml` pins `RUST_MSRV` by hand; a pin above
//! `workspace.package.rust-version` would silently gate a newer floor.

#![allow(clippy::tests_outside_test_module)]

const WORKFLOW: &str = "../../.github/workflows/msrv.yml";

fn declared_msrv(yaml: &str) -> Option<&str> {
    yaml.lines()
        .find_map(|line| line.trim().strip_prefix("RUST_MSRV:"))
        .map(|value| value.trim().trim_matches('"'))
}

#[test]
fn the_workflow_pins_the_manifest_msrv() {
    let manifest = env!("CARGO_PKG_RUST_VERSION");
    let yaml = std::fs::read_to_string(WORKFLOW)
        .unwrap_or_else(|e| panic!("failed to read {WORKFLOW}: {e}"));
    let pinned = declared_msrv(&yaml).unwrap_or_else(|| panic!("{WORKFLOW} declares no RUST_MSRV"));
    assert_eq!(
        pinned, manifest,
        "{WORKFLOW} pins RUST_MSRV {pinned}, workspace.package.rust-version is {manifest}"
    );
}

#[test]
fn a_commented_out_pin_does_not_count() {
    assert_eq!(
        declared_msrv("env:\n  # RUST_MSRV: \"1.0.0\"\n  RUST_MSRV: \"1.88.0\"\n"),
        Some("1.88.0")
    );
    assert_eq!(declared_msrv("env:\n  CARGO_TERM_COLOR: always\n"), None);
}
