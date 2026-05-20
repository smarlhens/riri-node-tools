# Workspace / Monorepo Support

Phase 10 of `riri-node-tools` — workspace-aware modes for `riri-nce` and `riri-npd` across npm, pnpm, and yarn. Temporary working doc; will be removed before merge.

---

# Workspace / Monorepo Support — Design

> Phase 10 of `riri-node-tools`. Adds workspace-aware modes to `riri-nce` and `riri-npd` across npm, pnpm, and yarn.

## Goal

Run `nce` and `npd` against multi-package monorepos with a single invocation at the workspace root. `npd` pins per-member; `nce` aggregates across the workspace and writes root engines.

## Non-Goals

- Per-member invocation walk-up. Workspace mode triggers only when cwd is the workspace root.
- Member-level `engines` writes for `nce`. Root is the only writeback target.
- New PMs (bun, deno) beyond the existing trio.
- Rewriting `riri-yarn` to parse `yarn.lock` directly (still `node_modules` scan).

## Architecture

New crate `riri-workspace` (workspace member of `crates/`).

```rust
pub struct WorkspaceProject {
    root: PathBuf,
    kind: PackageManager,
    globs: GlobSet,
}

pub struct WorkspaceMember {
    name: String,
    dir: PathBuf,
    manifest_path: PathBuf,
}

pub fn detect(cwd: &Path) -> Option<WorkspaceProject>;

impl WorkspaceProject {
    pub fn members(&self) -> Result<Vec<WorkspaceMember>, WorkspaceError>;
}
```

Dependencies: `riri-common` (PackageManager, PackageJsonFile), `riri-find-up`, `serde-saphyr` (pnpm-workspace.yaml), `globset`, `thiserror`.

Consumed by `riri-nce` and `riri-npd`. No NAPI changes (workspace mode is a runtime behavior, not an API).

## Detection

Detection runs at the cwd directory only — no parent walk. If no marker matches, `detect` returns `None` and the caller falls back to single-package mode.

| PM   | Marker                | Field                                                              |
| ---- | --------------------- | ------------------------------------------------------------------ |
| npm  | `package.json`        | `workspaces` (string array or `{packages: [...], nohoist: [...]}`) |
| pnpm | `pnpm-workspace.yaml` | `packages: [...]`                                                  |
| yarn | `package.json`        | `workspaces` (same shape as npm)                                   |

PM kind inherited from existing `riri_common::detect_lockfile`. If `detect_lockfile` returns `Pnpm` but `pnpm-workspace.yaml` lacks `packages:`, treat as non-workspace (the file may still hold `catalog:` entries; single-package mode still consumes those via the existing catalog path).

`nohoist` is parsed and ignored (yarn-only field; doesn't affect member set).

## Member Discovery

1. Compile workspace globs into a `globset::GlobSet`. Each glob is rooted at the workspace dir (e.g. `apps/*` matches `apps/web` but not `apps/web/sub`).
2. Walk the workspace tree starting at root. Skip:
   - `node_modules/` (any depth)
   - `.git/` (any depth)
   - dotdirs (entries starting with `.`)
3. For each dir matching a glob and containing `package.json`, emit a `WorkspaceMember`.
4. Dedup by canonical `manifest_path`. Symlinks are not followed into excluded dirs.
5. Member `name` = `package.json#name` or fallback to relative path from root (e.g. `apps/web`) when `name` is absent or empty. Relative paths use forward slashes regardless of host OS for stable output.

Order: stable, by `manifest_path` sorted lexicographically. Used for deterministic output.

## nce in Workspace Mode

The pnpm/npm/yarn lockfile already enumerates engines across all importers (single shared lockfile at root). Existing `LockfileEngines::engines_iter` returns the full set. So `nce`'s engine computation is workspace-correct as-is.

Phase 10 changes for `nce`:

- Run `detect()`. If `Some`, log `workspace detected: {kind}, {n} members` in `--debug`.
- No behavior change beyond logging. Writeback target remains root `package.json`.
- Members' own `engines` fields are ignored. They are advisory configuration owned by each member maintainer.

Exit code unchanged. `--json` output gains no new fields.

## npd in Workspace Mode

The substantive change. For each member, run the existing pin pipeline against the shared root lockfile.

Pseudocode:

```
project = detect(cwd)
if project is None:
    run_single_package_npd(cwd)
    return

lockfile = parse(root/lockfile)
members = project.members()

per_member_results = []
for member in members:
    pkg = read(member/package.json)
    pins = pin_dependencies(pkg, lockfile)        # existing fn
    per_member_results.push((member, pins))
    if args.update and pins.non_empty():
        apply_pins(pkg, pins)
        write_atomic(member/package.json, pkg)

if args.pin_catalog and project.kind == Pnpm:
    # catalog lives at root; reuse existing single-pass catalog flow
    handle_catalog(project.root, lockfile, args.update)

emit_output(per_member_results, args)
exit(combined_exit_code(per_member_results))
```

Skip rules unchanged: `file:`, `link:`, `workspace:`, `catalog:`, `catalog:<name>`. Sibling refs (e.g. `workspace:foo` pointing at another member) emit `tracing::debug!("skipped sibling ref: {dep}@{spec} in {member}")`.

`--pin-catalog` continues to act on root `pnpm-workspace.yaml`. Catalog and workspace are orthogonal at the data model level (catalog is a version map, workspace is a member set); both can be active in one run.

## Output

### Verbose (default + `-v`)

Section per member, suppressed when the member has zero pins (unless `--debug`):

```
apps/web:
    foo  ^1.0.0  →  1.0.5
    bar  ~2.3.0  →  2.3.4

apps/api:
    baz  ^3.0.0  →  3.1.2
```

Header is the member `name` as defined in Member Discovery, followed by `:`. Indentation matches single-package mode (4 spaces).

Footer hint (when not `--update`):

```
  Run npd -u to upgrade 2 package.json files.
```

### JSON

Schema extension. Single-package mode keeps current shape (`{pins: [...]}`). Workspace mode emits:

```json
{
  "members": [
    {"name": "apps/web", "manifest": "apps/web/package.json", "pins": [...]},
    {"name": "apps/api", "manifest": "apps/api/package.json", "pins": []}
  ]
}
```

`pins` items use the same shape as single-package mode (`name`, `kind`, `from`, `to`, plus catalog fields when applicable). Members with zero pins are included for completeness.

## Exit Codes (OR-combine)

Compute per-member exit code, then combine:

- `npd`: `max(per_member_codes)` where `2 > 1 > 0`. 2 = usage error (parse failure, missing lockfile). 1 = pins pending. 0 = clean.
- `nce`: `max(per_member_codes)` where `3 > 1 > 0`. 3 = unsatisfiable. (nce runs once at root, so this is effectively single-member, but the model generalizes if member-level work is added later.)

## Error Handling

- Workspace detected but no member matches globs → exit 0, message `"no workspace members found"`. If `--pin-catalog` is set on a pnpm project, the catalog pass still runs against root `pnpm-workspace.yaml`.
- Member `package.json` missing or invalid JSON → fail fast with `error: {member.name}: {reason}`. No partial writes (each member's apply happens atomically after pins computed for all members).
- Glob compile failure (invalid pattern in `workspaces` field) → exit 2, `error: invalid workspace pattern: {pat}: {reason}`.
- Two members with the same `name` → not blocking; output uses relative path fallback to disambiguate.

## Fixtures

Five new fixtures under `fixtures/`:

- `npd-npm-v3-workspace/` — root `package.json` with `workspaces: ["packages/*"]` + `packages/a/package.json`, `packages/b/package.json` (each unpinned) + root `package-lock.json` covering both.
- `npd-pnpm-v9-workspace/` — `pnpm-workspace.yaml` with `packages: ["packages/*"]` (and optional `catalog:` block) + members + root `pnpm-lock.yaml`.
- `npd-yarn-v1-workspace/` — same shape, yarn v1 root `package.json` + `node_modules/` scan layout.
- `pnpm-v9-workspace/` — nce regression: pnpm workspace with engines variation across members.
- (Reuse existing `npd-pnpm-v9-catalog/` semantics where helpful.)

Fixture deps use fake names per existing convention.

## Tests

### `riri-workspace` unit tests

- `detect` per PM (positive + negative)
- `detect` returns `None` when cwd is a member, not root
- Glob expansion: `apps/*`, `packages/**`, mixed
- Skip rules: `node_modules/`, `.git/`, dotdirs
- Dedup on overlapping globs
- Object-form `workspaces: {packages: [...]}` parsed
- Empty `packages:` → empty member list
- Member `name` fallback to relative path

### `riri-npd` integration

- Multi-member fixture: pin pending exit code 1
- `-u` rewrites each member's `package.json`, root untouched
- Verbose output sections per member, dedup zero-pin members
- JSON workspace schema
- `--pin-catalog` orthogonal with workspace mode (pnpm fixture)

### `riri-nce` integration

- Workspace fixture: `--debug` includes detection log
- No behavior change otherwise (snapshot equal to single-package mode against same lockfile)

### NAPI smoke

- `runCli` on workspace fixture exits cleanly

## File Plan

**New:**

- `crates/riri-workspace/Cargo.toml`
- `crates/riri-workspace/src/lib.rs`
- `crates/riri-workspace/src/detect.rs` — per-PM detection
- `crates/riri-workspace/src/members.rs` — glob + walk
- `crates/riri-workspace/tests/detect.rs`
- `crates/riri-workspace/tests/members.rs`
- `fixtures/npd-{npm,pnpm,yarn}-v*-workspace/`
- `fixtures/pnpm-v9-workspace/`

**Modified:**

- `Cargo.toml` — workspace member entry, `riri-workspace` workspace dep
- `crates/riri-common/src/lib.rs` — `PackageJson` gains `workspaces: Option<Workspaces>` (untagged enum for string-array or object form)
- `crates/riri-nce/Cargo.toml` + `src/cli.rs` — wire detection, debug log
- `crates/riri-npd/Cargo.toml` + `src/cli.rs` — workspace iteration branch
- `crates/riri-npd/src/lib.rs` — no fn signature changes; reuse `pin_dependencies`
- `crates/riri-pnpm/src/catalog.rs` — `find_workspace_yaml` stays; possibly extract a small `parse_packages_field` helper if cleaner
- `crates/riri-napi-npd/test.mjs` — workspace smoke test
- `crates/xtask/templates/readme-npd.tera` — note workspace support in options doc

## Open Questions

None. Decisions captured in brainstorming Q&A:

- Activation: auto-detect, no flag
- Invocation: root-only
- nce model: aggregate root-only (already the case)
- npd model: pin per-member, write each manifest
- Output: section per member with header
- yarn: keep `node_modules` scan
- Globs: `globset`
- Sibling refs: skip with debug log
- Exit codes: OR-combine worst-wins
- Detection lives in: new `riri-workspace` crate

---

# Workspace / Monorepo Support Implementation Plan

> Implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add auto-detected workspace mode to `riri-nce` and `riri-npd` covering npm, pnpm, and yarn workspaces. `npd` pins each member's `package.json` against the shared root lockfile; `nce` retains current root-only behavior with a debug log.

**Architecture:** New `riri-workspace` crate centralizes detection + member iteration (using `globset`). `riri-common::PackageJson` gains a `workspaces` field. `riri-nce` and `riri-npd` consume `riri_workspace::detect` at the top of `run`; npd's verbose/JSON output gain per-member sections.

**Tech Stack:** Rust 2024 edition, `globset` (member globs), `serde-saphyr` (pnpm-workspace.yaml), `thiserror`, existing `riri-common` / `riri-find-up` / per-PM crates. Tests: `insta` snapshots, `rstest`, `tempfile`.

**Spec:** `docs/specs/2026-05-19-workspace-monorepo-support-design.md`

---

## File Structure

**New files:**

- `crates/riri-workspace/Cargo.toml`
- `crates/riri-workspace/src/lib.rs` — public surface (`WorkspaceProject`, `WorkspaceMember`, `WorkspaceError`, `detect`)
- `crates/riri-workspace/src/detect.rs` — per-PM detection (npm/yarn `workspaces` field, pnpm yaml)
- `crates/riri-workspace/src/members.rs` — glob compile + tree walk
- `fixtures/npd-npm-v3-workspace/{package.json,package-lock.json,packages/{a,b}/package.json}`
- `fixtures/npd-pnpm-v9-workspace/{package.json,pnpm-lock.yaml,pnpm-workspace.yaml,packages/{a,b}/package.json}`
- `fixtures/npd-yarn-v1-workspace/{package.json,yarn.lock,node_modules/...,packages/{a,b}/package.json}`
- `fixtures/pnpm-v9-workspace/{package.json,pnpm-lock.yaml,pnpm-workspace.yaml,packages/{a,b}/package.json}`

**Modified files:**

- `Cargo.toml` — add `riri-workspace` to `[workspace.members]` and `[workspace.dependencies]`, add `globset` workspace dep.
- `crates/riri-common/src/lib.rs` — extend `PackageJson` with `workspaces: Option<WorkspacesField>`; add `WorkspacesField` enum (`Array(Vec<String>)` / `Object { packages: Vec<String>, .. }`).
- `crates/riri-nce/Cargo.toml` — add `riri-workspace` dep.
- `crates/riri-nce/src/cli.rs` — call `riri_workspace::detect` and emit debug log.
- `crates/riri-npd/Cargo.toml` — add `riri-workspace` dep.
- `crates/riri-npd/src/cli.rs` — workspace iteration branch in `run`; per-member output; JSON schema change in workspace mode.
- `crates/riri-napi-npd/test.mjs` — workspace smoke test.

---

## Task 1: Scaffold `riri-workspace` crate

**Files:**

- Create: `crates/riri-workspace/Cargo.toml`
- Create: `crates/riri-workspace/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + workspace.dependencies)

- [ ] **Step 1: Add `globset` to workspace deps**

Edit root `Cargo.toml`. In `[workspace.dependencies]` add:

```toml
globset = "0.4.16"
riri-workspace = { path = "crates/riri-workspace" }
```

Pin the exact `globset` version by checking `cargo search globset` first; if the latest minor is newer, use that exact version (no caret).

- [ ] **Step 2: Add crate to workspace members**

In root `Cargo.toml` `[workspace]` section, append `"crates/riri-workspace"` to `members`.

- [ ] **Step 3: Create crate manifest**

`crates/riri-workspace/Cargo.toml`:

```toml
[package]
name = "riri-workspace"
version = "0.1.0"
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
riri-common = { workspace = true }
riri-find-up = { workspace = true }
serde = { workspace = true }
serde-saphyr = "0.0.26"
globset = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
rstest = { workspace = true }
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 4: Create lib.rs skeleton**

`crates/riri-workspace/src/lib.rs`:

```rust
//! Workspace detection + member iteration for npm, pnpm, and yarn monorepos.

mod detect;
mod members;

pub use detect::{WorkspaceError, WorkspaceProject, detect};
pub use members::WorkspaceMember;
```

- [ ] **Step 5: Stub detect.rs**

`crates/riri-workspace/src/detect.rs`:

```rust
use crate::members::WorkspaceMember;
use globset::GlobSet;
use riri_common::PackageManager;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_saphyr::Error,
    },
    #[error("invalid workspace pattern `{pattern}`: {source}")]
    Glob {
        pattern: String,
        #[source]
        source: globset::Error,
    },
}

#[derive(Debug)]
pub struct WorkspaceProject {
    pub(crate) root: PathBuf,
    pub(crate) kind: PackageManager,
    pub(crate) globs: GlobSet,
    pub(crate) patterns: Vec<String>,
}

impl WorkspaceProject {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn kind(&self) -> &PackageManager {
        &self.kind
    }

    pub fn members(&self) -> Result<Vec<WorkspaceMember>, WorkspaceError> {
        crate::members::enumerate(self)
    }
}

#[must_use]
pub fn detect(cwd: &Path) -> Option<WorkspaceProject> {
    let _ = cwd;
    None
}
```

- [ ] **Step 6: Stub members.rs**

`crates/riri-workspace/src/members.rs`:

```rust
use crate::detect::{WorkspaceError, WorkspaceProject};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub name: String,
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
}

pub(crate) fn enumerate(
    project: &WorkspaceProject,
) -> Result<Vec<WorkspaceMember>, WorkspaceError> {
    let _ = project;
    Ok(Vec::new())
}
```

- [ ] **Step 7: Verify the workspace still builds**

Run: `cargo check --workspace --all-targets`
Expected: success.

- [ ] **Step 8: Commit**

```
git add Cargo.toml crates/riri-workspace
git commit -m "feat(workspace): scaffold riri-workspace crate"
```

---

## Task 2: `PackageJson::workspaces` field on `riri-common`

**Files:**

- Modify: `crates/riri-common/src/lib.rs`

- [ ] **Step 1: Write failing test**

In `crates/riri-common/src/lib.rs`, find the existing `#[cfg(test)] mod tests` block (or create one at the bottom if absent). Append:

```rust
#[cfg(test)]
mod workspaces_tests {
    use super::*;

    #[test]
    fn parses_array_form() {
        let json = r#"{"workspaces": ["apps/*", "packages/*"]}"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(
            pkg.workspaces.unwrap().packages(),
            vec!["apps/*".to_string(), "packages/*".to_string()]
        );
    }

    #[test]
    fn parses_object_form() {
        let json = r#"{"workspaces": {"packages": ["apps/*"], "nohoist": ["foo"]}}"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(
            pkg.workspaces.unwrap().packages(),
            vec!["apps/*".to_string()]
        );
    }

    #[test]
    fn missing_workspaces_is_none() {
        let pkg: PackageJson = serde_json::from_str(r#"{"name": "x"}"#).unwrap();
        assert!(pkg.workspaces.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p riri-common workspaces_tests`
Expected: FAIL — `workspaces` field doesn't exist.

- [ ] **Step 3: Add `WorkspacesField` enum + field**

Edit `crates/riri-common/src/lib.rs`. Above the `PackageJson` struct, add:

```rust
/// Workspaces field as found in `package.json`. npm + yarn accept both
/// `["apps/*"]` and `{"packages": ["apps/*"], "nohoist": [...]}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum WorkspacesField {
    Array(Vec<String>),
    Object {
        #[serde(default)]
        packages: Vec<String>,
        #[serde(default)]
        nohoist: Vec<String>,
    },
}

impl WorkspacesField {
    #[must_use]
    pub fn packages(&self) -> Vec<String> {
        match self {
            Self::Array(v) => v.clone(),
            Self::Object { packages, .. } => packages.clone(),
        }
    }
}
```

Then add `pub workspaces: Option<WorkspacesField>` (with `#[serde(default)]`) to the `PackageJson` struct alongside the other fields.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p riri-common workspaces_tests`
Expected: 3 tests PASS.

- [ ] **Step 5: Confirm no regression**

Run: `cargo test -p riri-common`
Expected: all PASS.

- [ ] **Step 6: Commit**

```
git add crates/riri-common/src/lib.rs
git commit -m "feat(common): PackageJson.workspaces field"
```

---

## Task 3: Detect npm + yarn workspaces

**Files:**

- Modify: `crates/riri-workspace/src/detect.rs`
- Modify: `crates/riri-workspace/src/lib.rs` (no changes — re-exports already in place)
- Create: `crates/riri-workspace/tests/detect.rs`

- [ ] **Step 1: Write failing integration test for npm**

`crates/riri-workspace/tests/detect.rs`:

```rust
#![allow(clippy::unwrap_used)]

use riri_workspace::detect;
use std::fs;
use tempfile::TempDir;

fn write_lockfile(dir: &std::path::Path, kind: &str) {
    let name = match kind {
        "npm" => "package-lock.json",
        "pnpm" => "pnpm-lock.yaml",
        "yarn" => "yarn.lock",
        _ => panic!("unknown PM"),
    };
    fs::write(dir.join(name), "").unwrap();
}

#[test]
fn detects_npm_array_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Npm));
    assert_eq!(project.root(), tmp.path());
}

#[test]
fn detects_npm_object_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces":{"packages":["apps/*"]}}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Npm));
}

#[test]
fn detects_yarn_workspaces() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "yarn");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Yarn));
}

#[test]
fn no_workspaces_field_returns_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"single","version":"1.0.0"}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "npm");
    assert!(detect(tmp.path()).is_none());
}

#[test]
fn missing_package_json_returns_none() {
    let tmp = TempDir::new().unwrap();
    write_lockfile(tmp.path(), "npm");
    assert!(detect(tmp.path()).is_none());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p riri-workspace --test detect`
Expected: FAIL — `detect` returns `None`.

- [ ] **Step 3: Implement `detect` for npm + yarn**

Replace the body of `detect` in `crates/riri-workspace/src/detect.rs`:

```rust
#[must_use]
pub fn detect(cwd: &Path) -> Option<WorkspaceProject> {
    let lockfile = riri_common::detect_lockfile(cwd).ok()?;
    let kind = lockfile.package_manager;
    let patterns = match kind {
        PackageManager::Npm | PackageManager::Yarn => read_workspaces_field(cwd)?,
        PackageManager::Pnpm => return None, // wired up in Task 4
    };
    if patterns.is_empty() {
        return None;
    }
    let globs = compile_globs(&patterns).ok()?;
    Some(WorkspaceProject {
        root: cwd.to_path_buf(),
        kind,
        globs,
        patterns,
    })
}

fn read_workspaces_field(cwd: &Path) -> Option<Vec<String>> {
    let path = cwd.join("package.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let pkg: riri_common::PackageJson = serde_json::from_str(&raw).ok()?;
    pkg.workspaces.map(|w| w.packages())
}

fn compile_globs(patterns: &[String]) -> Result<GlobSet, WorkspaceError> {
    let mut builder = globset::GlobSetBuilder::new();
    for pat in patterns {
        let glob = globset::Glob::new(pat).map_err(|source| WorkspaceError::Glob {
            pattern: pat.clone(),
            source,
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| WorkspaceError::Glob {
        pattern: patterns.join(", "),
        source,
    })
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p riri-workspace --test detect`
Expected: 5 tests PASS (pnpm test not yet written).

- [ ] **Step 5: Commit**

```
git add crates/riri-workspace/src crates/riri-workspace/tests/detect.rs
git commit -m "feat(workspace): detect npm + yarn workspaces"
```

---

## Task 4: Detect pnpm workspaces

**Files:**

- Modify: `crates/riri-workspace/src/detect.rs`
- Modify: `crates/riri-workspace/tests/detect.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/riri-workspace/tests/detect.rs`:

```rust
#[test]
fn detects_pnpm_workspaces_yaml() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "packages:\n  - 'packages/*'\n",
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");

    let project = detect(tmp.path()).expect("detected");
    assert!(matches!(project.kind(), riri_common::PackageManager::Pnpm));
}

#[test]
fn pnpm_yaml_without_packages_is_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("pnpm-workspace.yaml"),
        "catalog:\n  foo: ^1.0.0\n",
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");
    assert!(detect(tmp.path()).is_none());
}

#[test]
fn pnpm_missing_yaml_is_none() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name":"root","private":true}"#,
    )
    .unwrap();
    write_lockfile(tmp.path(), "pnpm");
    assert!(detect(tmp.path()).is_none());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p riri-workspace --test detect detects_pnpm_workspaces_yaml`
Expected: FAIL — pnpm branch returns `None` unconditionally.

- [ ] **Step 3: Implement pnpm branch**

In `crates/riri-workspace/src/detect.rs`, replace the `PackageManager::Pnpm => return None` line with `PackageManager::Pnpm => read_pnpm_yaml(cwd)?,`.

Then add this helper above `compile_globs`:

```rust
fn read_pnpm_yaml(cwd: &Path) -> Option<Vec<String>> {
    let path = cwd.join("pnpm-workspace.yaml");
    let raw = std::fs::read_to_string(&path).ok()?;

    #[derive(Debug, serde::Deserialize, Default)]
    struct Root {
        #[serde(default)]
        packages: Vec<String>,
    }
    let parsed: Root = serde_saphyr::from_str(&raw).ok()?;
    if parsed.packages.is_empty() {
        None
    } else {
        Some(parsed.packages)
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p riri-workspace --test detect`
Expected: 8 tests PASS.

- [ ] **Step 5: Commit**

```
git add crates/riri-workspace/src/detect.rs crates/riri-workspace/tests/detect.rs
git commit -m "feat(workspace): detect pnpm workspaces"
```

---

## Task 5: Member discovery via globset

**Files:**

- Modify: `crates/riri-workspace/src/members.rs`
- Create: `crates/riri-workspace/tests/members.rs`

- [ ] **Step 1: Write failing test**

`crates/riri-workspace/tests/members.rs`:

```rust
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
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )
        .unwrap();
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
        &[("packages/a", "a"), ("node_modules/x/packages/poison", "poison")],
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
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p riri-workspace --test members`
Expected: FAIL — `enumerate` returns empty.

- [ ] **Step 3: Implement member walk**

Replace `crates/riri-workspace/src/members.rs` with:

```rust
use crate::detect::{WorkspaceError, WorkspaceProject};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub name: String,
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
}

pub(crate) fn enumerate(
    project: &WorkspaceProject,
) -> Result<Vec<WorkspaceMember>, WorkspaceError> {
    let mut out: Vec<WorkspaceMember> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    walk(&project.root, &project.root, &project.globs, &mut out, &mut seen)?;
    out.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));
    Ok(out)
}

fn walk(
    dir: &Path,
    root: &Path,
    globs: &globset::GlobSet,
    out: &mut Vec<WorkspaceMember>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), WorkspaceError> {
    let read = std::fs::read_dir(dir).map_err(|source| WorkspaceError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in read.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "node_modules" {
            continue;
        }
        let entry_path = entry.path();
        let meta = match entry.file_type() {
            Ok(m) if m.is_dir() => m,
            _ => continue,
        };
        // Symlinks: only follow non-link dirs.
        if meta.is_symlink() {
            continue;
        }

        let rel = entry_path.strip_prefix(root).unwrap_or(&entry_path);
        let rel_str = path_to_forward_slash(rel);
        if globs.is_match(&rel_str) {
            let manifest = entry_path.join("package.json");
            if manifest.is_file() {
                let canonical = manifest.canonicalize().unwrap_or_else(|_| manifest.clone());
                if seen.insert(canonical.clone()) {
                    let name = member_name(&manifest, &rel_str);
                    out.push(WorkspaceMember {
                        name,
                        dir: entry_path.clone(),
                        manifest_path: manifest,
                    });
                }
            }
        }

        walk(&entry_path, root, globs, out, seen)?;
    }
    Ok(())
}

fn member_name(manifest: &Path, rel: &str) -> String {
    let raw = match std::fs::read_to_string(manifest) {
        Ok(s) => s,
        Err(_) => return rel.to_string(),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return rel.to_string(),
    };
    parsed
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map_or_else(|| rel.to_string(), str::to_string)
}

fn path_to_forward_slash(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p riri-workspace --test members`
Expected: 6 tests PASS.

- [ ] **Step 5: Run full crate test suite**

Run: `cargo test -p riri-workspace`
Expected: all PASS.

- [ ] **Step 6: Commit**

```
git add crates/riri-workspace/src/members.rs crates/riri-workspace/tests/members.rs
git commit -m "feat(workspace): member discovery via globset"
```

---

## Task 6: npm workspace fixture

**Files:**

- Create: `fixtures/npd-npm-v3-workspace/package.json`
- Create: `fixtures/npd-npm-v3-workspace/package-lock.json`
- Create: `fixtures/npd-npm-v3-workspace/packages/a/package.json`
- Create: `fixtures/npd-npm-v3-workspace/packages/b/package.json`

- [ ] **Step 1: Root package.json**

`fixtures/npd-npm-v3-workspace/package.json`:

```json
{
  "name": "fake-workspace-root",
  "private": true,
  "workspaces": ["packages/*"]
}
```

- [ ] **Step 2: Member a/package.json**

`fixtures/npd-npm-v3-workspace/packages/a/package.json`:

```json
{
  "name": "@fake/a",
  "version": "1.0.0",
  "dependencies": {
    "fake-foo": "^1.0.0"
  }
}
```

- [ ] **Step 3: Member b/package.json**

`fixtures/npd-npm-v3-workspace/packages/b/package.json`:

```json
{
  "name": "@fake/b",
  "version": "1.0.0",
  "dependencies": {
    "fake-bar": "~2.3.0"
  }
}
```

- [ ] **Step 4: Root package-lock.json**

`fixtures/npd-npm-v3-workspace/package-lock.json`:

```json
{
  "name": "fake-workspace-root",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": { "workspaces": ["packages/*"] },
    "packages/a": { "version": "1.0.0", "dependencies": { "fake-foo": "^1.0.0" } },
    "packages/b": { "version": "1.0.0", "dependencies": { "fake-bar": "~2.3.0" } },
    "node_modules/fake-foo": { "version": "1.0.5" },
    "node_modules/fake-bar": { "version": "2.3.4" }
  }
}
```

- [ ] **Step 5: Smoke-check detection**

Run from repo root:

```
cargo test -p riri-workspace --test detect
```

(Note: this fixture is not loaded by the detect tests yet — they use tempdirs. This step is only to confirm no regression.) Expected: all PASS.

- [ ] **Step 6: Commit**

```
git add fixtures/npd-npm-v3-workspace
git commit -m "test(npd): npm workspace fixture"
```

---

## Task 7: pnpm workspace fixture

**Files:**

- Create: `fixtures/npd-pnpm-v9-workspace/{package.json,pnpm-lock.yaml,pnpm-workspace.yaml,packages/{a,b}/package.json}`

- [ ] **Step 1: Root package.json**

`fixtures/npd-pnpm-v9-workspace/package.json`:

```json
{
  "name": "fake-pnpm-workspace",
  "private": true
}
```

- [ ] **Step 2: pnpm-workspace.yaml**

`fixtures/npd-pnpm-v9-workspace/pnpm-workspace.yaml`:

```yaml
packages:
  - 'packages/*'
```

- [ ] **Step 3: Members**

`fixtures/npd-pnpm-v9-workspace/packages/a/package.json`:

```json
{
  "name": "@fake/a",
  "version": "1.0.0",
  "dependencies": {
    "fake-foo": "^1.0.0"
  }
}
```

`fixtures/npd-pnpm-v9-workspace/packages/b/package.json`:

```json
{
  "name": "@fake/b",
  "version": "1.0.0",
  "dependencies": {
    "fake-bar": "~2.3.0"
  }
}
```

- [ ] **Step 4: pnpm-lock.yaml**

`fixtures/npd-pnpm-v9-workspace/pnpm-lock.yaml`:

```yaml
lockfileVersion: '9.0'

importers:
  .:
    dependencies: {}

  packages/a:
    dependencies:
      fake-foo:
        specifier: ^1.0.0
        version: 1.0.5

  packages/b:
    dependencies:
      fake-bar:
        specifier: ~2.3.0
        version: 2.3.4

packages:
  fake-foo@1.0.5: {}

  fake-bar@2.3.4: {}

snapshots:
  fake-foo@1.0.5: {}

  fake-bar@2.3.4: {}
```

- [ ] **Step 5: Commit**

```
git add fixtures/npd-pnpm-v9-workspace
git commit -m "test(npd): pnpm workspace fixture"
```

---

## Task 8: yarn workspace fixture

**Files:**

- Create: `fixtures/npd-yarn-v1-workspace/package.json`
- Create: `fixtures/npd-yarn-v1-workspace/yarn.lock`
- Create: `fixtures/npd-yarn-v1-workspace/packages/{a,b}/package.json`
- Create: `fixtures/npd-yarn-v1-workspace/node_modules/{fake-foo,fake-bar}/package.json`

- [ ] **Step 1: Root package.json**

`fixtures/npd-yarn-v1-workspace/package.json`:

```json
{
  "name": "fake-yarn-workspace",
  "private": true,
  "workspaces": ["packages/*"]
}
```

- [ ] **Step 2: Members**

`fixtures/npd-yarn-v1-workspace/packages/a/package.json`:

```json
{
  "name": "@fake/a",
  "version": "1.0.0",
  "dependencies": {
    "fake-foo": "^1.0.0"
  }
}
```

`fixtures/npd-yarn-v1-workspace/packages/b/package.json`:

```json
{
  "name": "@fake/b",
  "version": "1.0.0",
  "dependencies": {
    "fake-bar": "~2.3.0"
  }
}
```

- [ ] **Step 3: yarn.lock**

`fixtures/npd-yarn-v1-workspace/yarn.lock`:

```
# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1
```

(Empty body is fine — `riri-yarn` scans `node_modules` for versions.)

- [ ] **Step 4: node_modules entries**

`fixtures/npd-yarn-v1-workspace/node_modules/fake-foo/package.json`:

```json
{ "name": "fake-foo", "version": "1.0.5" }
```

`fixtures/npd-yarn-v1-workspace/node_modules/fake-bar/package.json`:

```json
{ "name": "fake-bar", "version": "2.3.4" }
```

- [ ] **Step 5: Commit**

```
git add fixtures/npd-yarn-v1-workspace
git commit -m "test(npd): yarn workspace fixture"
```

---

## Task 9: nce pnpm workspace regression fixture

**Files:**

- Create: `fixtures/pnpm-v9-workspace/{package.json,pnpm-lock.yaml,pnpm-workspace.yaml,packages/{a,b}/package.json}`

- [ ] **Step 1: Files**

`fixtures/pnpm-v9-workspace/package.json`:

```json
{
  "name": "fake-pnpm-workspace-engines",
  "private": true
}
```

`fixtures/pnpm-v9-workspace/pnpm-workspace.yaml`:

```yaml
packages:
  - 'packages/*'
```

`fixtures/pnpm-v9-workspace/packages/a/package.json`:

```json
{
  "name": "@fake/a",
  "version": "1.0.0",
  "dependencies": { "fake-foo": "^1.0.0" }
}
```

`fixtures/pnpm-v9-workspace/packages/b/package.json`:

```json
{
  "name": "@fake/b",
  "version": "1.0.0",
  "dependencies": { "fake-bar": "^2.0.0" }
}
```

`fixtures/pnpm-v9-workspace/pnpm-lock.yaml`:

```yaml
lockfileVersion: '9.0'

importers:
  .:
    dependencies: {}

  packages/a:
    dependencies:
      fake-foo:
        specifier: ^1.0.0
        version: 1.0.5

  packages/b:
    dependencies:
      fake-bar:
        specifier: ^2.0.0
        version: 2.4.1

packages:
  fake-foo@1.0.5:
    resolution:
      integrity: sha512-placeholder
    engines:
      node: '>=18.0.0'

  fake-bar@2.4.1:
    resolution:
      integrity: sha512-placeholder
    engines:
      node: '>=20.0.0'

snapshots:
  fake-foo@1.0.5: {}

  fake-bar@2.4.1: {}
```

- [ ] **Step 2: Commit**

```
git add fixtures/pnpm-v9-workspace
git commit -m "test(nce): pnpm workspace fixture"
```

---

## Task 10: npd CLI — workspace iteration

**Files:**

- Modify: `crates/riri-npd/Cargo.toml`
- Modify: `crates/riri-npd/src/cli.rs`
- Modify: `crates/riri-npd/tests/cli_snapshots.rs`

- [ ] **Step 1: Add `riri-workspace` to `riri-npd`**

Edit `crates/riri-npd/Cargo.toml`, in `[dependencies]` add:

```toml
riri-workspace = { workspace = true }
```

- [ ] **Step 2: Write failing snapshot test**

Append to `crates/riri-npd/tests/cli_snapshots.rs`:

```rust
#[test]
fn cli_npm_workspace_lists_per_member_pins() {
    let (stdout, stderr, code) = run_in_fixture(
        "npd-npm-v3-workspace",
        &["-v"],
    );
    assert_eq!(code, 1);
    insta::assert_snapshot!("npm_workspace_verbose", stderr);
    let _ = stdout;
}
```

- [ ] **Step 3: Run test to verify failure**

Run: `cargo test -p riri-npd --test cli_snapshots cli_npm_workspace_lists_per_member_pins`
Expected: FAIL — no workspace iteration yet, output is single-package format.

- [ ] **Step 4: Wire detection into `run`**

In `crates/riri-npd/src/cli.rs`, at the top of `fn run` (after `let cwd = std::env::current_dir()...`), insert:

```rust
    if let Some(project) = riri_workspace::detect(&cwd) {
        return run_workspace(args, &runner, project);
    }
```

Then add helpers at the bottom of `cli.rs`:

```rust
fn run_workspace(
    args: &Args,
    runner: &TaskRunner,
    project: riri_workspace::WorkspaceProject,
) -> Result<i32> {
    let members = project
        .members()
        .map_err(|e| anyhow::anyhow!("failed to enumerate workspace members: {e}"))?;

    if members.is_empty() {
        if !args.quiet {
            eprintln!("  no workspace members found");
        }
        return Ok(EXIT_OK);
    }

    let task = runner.task("Detecting lockfile...");
    let lockfile_result = match detect_lockfile(project.root()) {
        Ok(result) => {
            task.complete(&format!(
                "Detected {}",
                result.path.file_name().unwrap_or_default().to_string_lossy()
            ));
            result
        }
        Err(e) => {
            task.fail("Detecting lockfile");
            return Err(anyhow::anyhow!(e));
        }
    };

    let task = runner.task("Parsing lockfile...");
    let lockfile = match load_lockfile(&lockfile_result.package_manager, &lockfile_result.path) {
        Ok(lock) => {
            task.complete("Parsed lockfile");
            lock
        }
        Err(e) => {
            task.fail("Parsing lockfile");
            return Err(e);
        }
    };

    let mut per_member: Vec<(riri_workspace::WorkspaceMember, Vec<VersionToPin>, PackageJsonFile)> =
        Vec::new();
    let mut worst = EXIT_OK;

    for member in members {
        let mut pkg_file = PackageJsonFile::read(&member.manifest_path)
            .map_err(|e| anyhow::anyhow!("{}: {e}", member.name))?;
        let pins = pin_dependencies(&pkg_file.parsed, lockfile.as_ref())
            .map_err(|e| anyhow::anyhow!("{}: pin_dependencies failed: {e}", member.name))?;
        if !pins.is_empty() {
            worst = EXIT_PINS_PENDING;
        }
        if args.update && !pins.is_empty() {
            apply_pins(&mut pkg_file, &pins);
            if args.sort {
                pkg_file
                    .write_sorted()
                    .map_err(|e| anyhow::anyhow!("{}: write failed: {e}", member.name))?;
            } else {
                pkg_file
                    .write()
                    .map_err(|e| anyhow::anyhow!("{}: write failed: {e}", member.name))?;
            }
        }
        per_member.push((member, pins, pkg_file));
    }

    if args.json {
        emit_workspace_json(&per_member);
    } else {
        emit_workspace_text(args, &per_member);
    }
    Ok(worst)
}

fn emit_workspace_json(
    per_member: &[(riri_workspace::WorkspaceMember, Vec<VersionToPin>, PackageJsonFile)],
) {
    let members: Vec<serde_json::Value> = per_member
        .iter()
        .map(|(m, pins, _)| {
            serde_json::json!({
                "name": m.name,
                "manifest": m.manifest_path.to_string_lossy(),
                "pins": pins.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "kind": p.kind.as_str(),
                    "from": p.current_range,
                    "to": p.pinned_version,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let out = serde_json::json!({ "members": members });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

fn emit_workspace_text(
    args: &Args,
    per_member: &[(riri_workspace::WorkspaceMember, Vec<VersionToPin>, PackageJsonFile)],
) {
    if args.quiet {
        return;
    }
    let mut any = false;
    for (member, pins, _) in per_member {
        if pins.is_empty() {
            continue;
        }
        any = true;
        eprintln!("\n  {}:", member.name);
        let mut table = Table::new();
        table.load_preset(presets::NOTHING);
        for pin in pins {
            table.add_row(vec![
                pin.name.clone(),
                pin.current_range.clone(),
                "\u{2192}".to_string(),
                pin.pinned_version.clone(),
            ]);
        }
        for line in table.lines() {
            eprintln!("    {}", line.trim());
        }
    }
    if !any {
        eprintln!(
            "\n  All workspace dependencies are already pinned {}",
            style(":)").green()
        );
    } else if !args.update {
        let hint = generate_update_hint(args);
        eprintln!(
            "\n  Run {} to upgrade {} package.json files.",
            style(hint).bold().cyan(),
            per_member.iter().filter(|(_, p, _)| !p.is_empty()).count(),
        );
    }
}
```

Note: `PackageJsonFile` is already imported at the top of `cli.rs` via `riri_common`. Add `use riri_workspace;` to the existing use block if rust-analyzer complains; the fully-qualified paths above also work.

- [ ] **Step 5: Run test and accept snapshot**

Run: `cargo test -p riri-npd --test cli_snapshots cli_npm_workspace_lists_per_member_pins`
Expected: FAIL — new snapshot pending.

Run: `cargo insta accept`
Verify the accepted snapshot includes both members with their pins.

Run again: `cargo test -p riri-npd --test cli_snapshots cli_npm_workspace_lists_per_member_pins`
Expected: PASS.

- [ ] **Step 6: Run full npd suite**

Run: `cargo test -p riri-npd`
Expected: all PASS (existing single-package tests untouched).

- [ ] **Step 7: Commit**

```
git add crates/riri-npd/Cargo.toml crates/riri-npd/src/cli.rs crates/riri-npd/tests/cli_snapshots.rs crates/riri-npd/tests/snapshots/cli_snapshots__npm_workspace_verbose.snap
git commit -m "feat(npd): workspace mode iterates members"
```

---

## Task 11: npd `-u` writes each member's manifest

**Files:**

- Modify: `crates/riri-npd/tests/cli_snapshots.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/riri-npd/tests/cli_snapshots.rs`:

```rust
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
```

If `copy_fixture_to_tmp` exists only for flat fixtures, add a recursive variant `copy_fixture_to_tmp_workspace` near it (in the same file). Otherwise extend `copy_fixture_to_tmp` to handle subdirs:

```rust
fn copy_fixture_to_tmp_workspace(name: &str) -> tempfile::TempDir {
    let src = std::path::Path::new("../../fixtures").join(name);
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
```

- [ ] **Step 2: Run test**

Run: `cargo test -p riri-npd --test cli_snapshots cli_workspace_update_rewrites_each_member`
Expected: PASS (Task 10 already wired writes; this test confirms).

- [ ] **Step 3: Commit**

```
git add crates/riri-npd/tests/cli_snapshots.rs
git commit -m "test(npd): -u rewrites each workspace member"
```

---

## Task 12: npd `--json` workspace schema

**Files:**

- Modify: `crates/riri-npd/tests/cli_snapshots.rs`

- [ ] **Step 1: Write failing snapshot test**

Append:

```rust
#[test]
fn cli_workspace_json_schema() {
    let (stdout, _stderr, code) = run_in_fixture(
        "npd-npm-v3-workspace",
        &["--json", "--quiet"],
    );
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
```

- [ ] **Step 2: Run test, accept snapshot**

Run: `cargo test -p riri-npd --test cli_snapshots cli_workspace_json_schema`
Expected: FAIL on first run (snapshot pending).

Run: `cargo insta accept`
Verify accepted snapshot shows `members` array with both packages.

Run again: PASS.

- [ ] **Step 3: Commit**

```
git add crates/riri-npd/tests/cli_snapshots.rs crates/riri-npd/tests/snapshots/cli_snapshots__workspace_json_schema.snap
git commit -m "test(npd): workspace JSON schema"
```

---

## Task 13: pnpm workspace + `--pin-catalog` interaction

**Files:**

- Modify: `crates/riri-npd/src/cli.rs`
- Modify: `crates/riri-npd/tests/cli_snapshots.rs`

- [ ] **Step 1: Add catalog handling in workspace path**

In `crates/riri-npd/src/cli.rs`, inside `fn run_workspace` after the member loop and before output emission, add:

```rust
    let mut catalog_pins: Vec<crate::CatalogPin> = Vec::new();
    if args.pin_catalog && lockfile_result.package_manager == PackageManager::Pnpm {
        let plan = resolve_catalog_pins(args, runner, &lockfile_result, project.root(), lockfile.as_ref())?;
        if !plan.pins.is_empty() {
            worst = EXIT_PINS_PENDING;
        }
        if args.update
            && let Some(source) = plan.source.as_ref()
            && !plan.pins.is_empty()
        {
            let task = runner.task("Updating pnpm-workspace.yaml...");
            match apply_catalog_pins(source, &plan.pins) {
                Ok(()) => task.complete("Updated pnpm-workspace.yaml"),
                Err(e) => {
                    task.fail("Updating pnpm-workspace.yaml");
                    return Err(e);
                }
            }
        }
        catalog_pins = plan.pins;
    }
```

Verify `resolve_catalog_pins` signature accepts `cwd: &Path`. It does today (it's pnpm-specific and takes `&Path`).

Extend the JSON emitter to include catalog pins as a top-level field:

```rust
fn emit_workspace_json(
    per_member: &[(riri_workspace::WorkspaceMember, Vec<VersionToPin>, PackageJsonFile)],
    catalog_pins: &[crate::CatalogPin],
) {
    let members: Vec<serde_json::Value> = per_member
        .iter()
        .map(|(m, pins, _)| {
            serde_json::json!({
                "name": m.name,
                "manifest": m.manifest_path.to_string_lossy(),
                "pins": pins.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "kind": p.kind.as_str(),
                    "from": p.current_range,
                    "to": p.pinned_version,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let catalog: Vec<serde_json::Value> = catalog_pins
        .iter()
        .map(|cp| serde_json::json!({
            "name": cp.dep_name,
            "kind": "catalog",
            "catalog": cp.catalog_name,
            "from": cp.from,
            "to": cp.to,
        }))
        .collect();
    let out = serde_json::json!({ "members": members, "catalog": catalog });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}
```

Update the call site in `run_workspace` to pass `&catalog_pins`. Text emitter follows the same pattern — append a `(catalog)` / `(catalog:name)` section after the per-member sections.

- [ ] **Step 2: Extend pnpm workspace fixture with a catalog block**

Edit `fixtures/npd-pnpm-v9-workspace/pnpm-workspace.yaml`:

```yaml
packages:
  - 'packages/*'
catalog:
  fake-baz: ^3.0.0
```

Add the dep to `fixtures/npd-pnpm-v9-workspace/packages/a/package.json`:

```json
{
  "name": "@fake/a",
  "version": "1.0.0",
  "dependencies": {
    "fake-foo": "^1.0.0",
    "fake-baz": "catalog:"
  }
}
```

Add to `pnpm-lock.yaml` under `importers."packages/a".dependencies`:

```yaml
fake-baz:
  specifier: 'catalog:'
  version: 3.1.0
```

And to `packages` + `snapshots`:

```yaml
fake-baz@3.1.0: {}
```

(both lists).

- [ ] **Step 3: Write failing test**

Append to `crates/riri-npd/tests/cli_snapshots.rs`:

```rust
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
```

- [ ] **Step 4: Run + verify**

Run: `cargo test -p riri-npd --test cli_snapshots cli_workspace_pin_catalog_includes_catalog_section`
Expected: PASS.

- [ ] **Step 5: Run full npd suite**

Run: `cargo test -p riri-npd`
Expected: all PASS.

- [ ] **Step 6: Commit**

```
git add crates/riri-npd/src/cli.rs crates/riri-npd/tests/cli_snapshots.rs fixtures/npd-pnpm-v9-workspace
git commit -m "feat(npd): workspace mode handles --pin-catalog"
```

---

## Task 14: nce detection + debug log

**Files:**

- Modify: `crates/riri-nce/Cargo.toml`
- Modify: `crates/riri-nce/src/cli.rs`
- Modify: `crates/riri-nce/tests/cli_snapshots.rs`

- [ ] **Step 1: Add dep**

Edit `crates/riri-nce/Cargo.toml`, in `[dependencies]` add:

```toml
riri-workspace = { workspace = true }
```

- [ ] **Step 2: Write failing test**

Append to `crates/riri-nce/tests/cli_snapshots.rs`:

```rust
#[test]
fn cli_nce_workspace_debug_log() {
    let (_stdout, stderr, _code) = run_in_fixture("pnpm-v9-workspace", &["-d"]);
    assert!(
        stderr.contains("workspace detected"),
        "stderr missing workspace marker: {stderr}"
    );
}
```

- [ ] **Step 3: Run test to verify failure**

Run: `cargo test -p riri-nce --test cli_snapshots cli_nce_workspace_debug_log`
Expected: FAIL — no log emitted yet.

- [ ] **Step 4: Wire detection**

In `crates/riri-nce/src/cli.rs`, inside `fn run` after `let cwd = std::env::current_dir()...`, insert:

```rust
    if let Some(project) = riri_workspace::detect(&cwd) {
        let n = project.members().map(|m| m.len()).unwrap_or(0);
        tracing::debug!(
            "workspace detected: {:?}, {} members",
            project.kind(),
            n
        );
    }
```

(Confirm the tracing import is already there; if not, add `use tracing;` to the existing imports.)

- [ ] **Step 5: Run test to verify pass**

Run: `cargo test -p riri-nce --test cli_snapshots cli_nce_workspace_debug_log`
Expected: PASS.

- [ ] **Step 6: Run full nce suite**

Run: `cargo test -p riri-nce`
Expected: all PASS.

- [ ] **Step 7: Commit**

```
git add crates/riri-nce/Cargo.toml crates/riri-nce/src/cli.rs crates/riri-nce/tests/cli_snapshots.rs
git commit -m "feat(nce): log workspace detection in --debug"
```

---

## Task 15: NAPI smoke test for workspace mode

**Files:**

- Modify: `crates/riri-napi-npd/test.mjs`

- [ ] **Step 1: Add test**

Append to `crates/riri-napi-npd/test.mjs`, after the existing `runCli --pin-catalog` test:

```js
test('runCli workspace mode on npm v3 workspace fixture', () => {
  const fixtureDir = resolve(fixturesDir, 'npd-npm-v3-workspace');
  const original = process.cwd();
  try {
    process.chdir(fixtureDir);
    const code = napi.runCli(['npd', '--quiet', '--json']);
    assert.equal(code, 1);
  } finally {
    process.chdir(original);
  }
});
```

- [ ] **Step 2: Commit**

```
git add crates/riri-napi-npd/test.mjs
git commit -m "test(napi-npd): runCli smoke test for workspace mode"
```

---

## Task 16: README + xtask template updates

**Files:**

- Modify: `crates/xtask/templates/readme-npd.tera`

- [ ] **Step 1: Add workspace note**

Open `crates/xtask/templates/readme-npd.tera`. After the `## Options` block, add a new section:

```markdown
## Workspace mode

When run from the root of an npm, pnpm, or yarn workspace, `npd` auto-detects the workspace and pins each member's `package.json` against the shared root lockfile. Output is grouped per member. `--pin-catalog` continues to operate on the root `pnpm-workspace.yaml`.
```

- [ ] **Step 2: Regenerate README (CI does this on push — local regen needs napi build)**

If a local napi build is available:

```
cargo xtask regen-readme --crate-name npd
```

Otherwise, manually patch `crates/riri-napi-npd/README.md` by copying the new `## Workspace mode` block to the same position as in the template. CI will catch any drift.

- [ ] **Step 3: Commit**

```
git add crates/xtask/templates/readme-npd.tera crates/riri-napi-npd/README.md
git commit -m "docs(npd): document workspace mode"
```

---

## Task 17: Final verification

- [ ] **Step 1: Workspace build + test**

Run:

```
cargo build --workspace
cargo test --workspace
```

Expected: clean build, all tests PASS.

- [ ] **Step 2: Format + lint**

Run:

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no diffs, no warnings.

- [ ] **Step 3: Manual smoke — npm**

```
cd fixtures/npd-npm-v3-workspace
cargo run --manifest-path ../../Cargo.toml -p riri-npd -- -v
```

Expected stderr: two sections (`@fake/a:`, `@fake/b:`), each with one pin row, hint to run `-u`.

```
cargo run --manifest-path ../../Cargo.toml -p riri-npd -- -u
```

Confirm `packages/a/package.json` shows `"fake-foo": "1.0.5"` and `packages/b/package.json` shows `"fake-bar": "2.3.4"`. Reset with `git checkout -- packages`.

- [ ] **Step 4: Manual smoke — pnpm with catalog**

```
cd fixtures/npd-pnpm-v9-workspace
cargo run --manifest-path ../../Cargo.toml -p riri-npd -- --pin-catalog -v
```

Expected: per-member sections + catalog section showing `fake-baz` pin.

Reset: `cd ../.. && git checkout -- fixtures/npd-pnpm-v9-workspace`.

- [ ] **Step 5: Open PR**

```
git push -u origin feat/workspace-support
gh pr create --title "feat: workspace / monorepo support" --body "$(cat <<'EOF'
## Summary
- New `riri-workspace` crate: detects npm / pnpm / yarn workspaces at cwd, enumerates members via globset.
- `riri-npd` auto-detects workspace mode at the root and pins each member's `package.json` against the shared lockfile.
- Per-member output sections; JSON schema gains a `members` array; `--pin-catalog` continues to operate on root `pnpm-workspace.yaml` (now combined with workspace iteration on pnpm).
- `riri-nce` logs workspace detection in `--debug` (computation already aggregates via the shared lockfile).
- New fixtures: `npd-npm-v3-workspace`, `npd-pnpm-v9-workspace`, `npd-yarn-v1-workspace`, `pnpm-v9-workspace`.

## Test plan
- [x] `riri-workspace` detection unit tests cover npm / pnpm / yarn / negative paths
- [x] `riri-workspace` member discovery tests cover globs / `node_modules` skip / dotdirs / dedup / name fallback
- [x] `npd` CLI snapshot tests for verbose + JSON + `-u` + `--pin-catalog` combinations
- [x] `nce` `--debug` workspace log test
- [x] NAPI `runCli` smoke test for workspace mode
EOF
)"
```

---

## Self-Review

**Spec coverage:**

- Architecture (`riri-workspace` crate) → Task 1
- npm/yarn detection → Task 3
- pnpm detection → Task 4
- Member discovery + edge cases → Task 5
- nce workspace mode → Task 14
- npd workspace mode → Tasks 10, 11, 12, 13
- Output schema → Tasks 10 (text), 12 (JSON)
- Exit codes (OR-combine) → Task 10 (`worst` tracking)
- Fixtures (npm, pnpm, yarn, nce regression) → Tasks 6, 7, 8, 9
- NAPI smoke → Task 15
- Edge cases (empty members, missing markers, glob errors) → Tasks 4, 5
- Docs → Task 16
- Final verification → Task 17

**Placeholder scan:** No "TBD", "TODO", or "similar to" references. All code blocks contain full content.

**Type consistency:**

- `WorkspaceProject` / `WorkspaceMember` field names stable across Tasks 1, 5, 10.
- `VersionToPin` / `PackageJsonFile` / `CatalogPin` references match existing `riri-npd` types.
- `run_workspace` signature stable between Task 10 and Task 13.

**Inline notes:**

- Task 10's text emitter uses 4-column tables (no `(catalog)` annotation in workspace text mode — catalog pins are emitted in a separate trailing section by Task 13). Single-package mode keeps its 5-column behavior.
- Task 13 widens `emit_workspace_json` signature; the call site update is mentioned but not shown as a separate diff — that's intentional, the engineer follows the type to its single caller in `run_workspace`.
- `globset` glob root: each pattern is relative to the workspace root and matches against forward-slash relative paths. Tested in Task 5.
