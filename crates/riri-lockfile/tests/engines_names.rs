#![allow(clippy::tests_outside_test_module)]

//! `LockfileEngines::engines_iter` promises bare package names. The three
//! implementations live in downstream crates, so the invariant is checked here —
//! the one crate that can see all of them — over the whole fixture corpus.

use riri_common::PackageManager;
use riri_lockfile::parse_lockfile;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate manifest under workspace")
        .to_path_buf()
}

#[test]
fn engines_iter_yields_bare_package_names() {
    let fixtures = workspace_root().join("fixtures");
    let mut names = 0_usize;

    for entry in std::fs::read_dir(&fixtures).expect("read fixtures dir") {
        let dir = entry.expect("fixture entry").path();
        if !dir.is_dir() {
            continue;
        }
        for manager in PackageManager::ALL {
            let lockfile = dir.join(manager.lockfile_name());
            if !lockfile.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&lockfile).expect("read lockfile");
            let Ok(parsed) = parse_lockfile(manager, &content) else {
                continue;
            };
            let Some(engines) = parsed.engines() else {
                continue;
            };

            for (name, _) in engines.engines_iter() {
                let at = || format!("{name} in {}", lockfile.display());
                assert!(
                    !name.contains("node_modules"),
                    "path prefix left in: {}",
                    at()
                );
                assert!(!name.starts_with('/'), "leading slash left in: {}", at());
                assert!(!name.contains('('), "peer suffix left in: {}", at());
                assert!(
                    !name.get(1..).is_some_and(|rest| rest.contains('@')),
                    "version suffix left in: {}",
                    at()
                );
                names += 1;
            }
        }
    }

    assert!(
        names > 50,
        "fixture corpus should exercise this heavily, saw {names} names"
    );
}
