//! Regenerate napi-crate READMEs from full Tera templates.

use anyhow::{Context, anyhow, bail};
use similar::TextDiff;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tera::{Context as TeraContext, Tera};

const TPL_README_NCE: &str = include_str!("../templates/readme-nce.tera");
const TPL_README_NCD: &str = include_str!("../templates/readme-ncd.tera");
const TPL_README_NPD: &str = include_str!("../templates/readme-npd.tera");
const TPL_README_ROOT: &str = include_str!("../templates/readme-root.tera");

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Fail with a non-zero exit code and print a diff if the README would change.
    #[arg(long)]
    pub check: bool,
    /// Restrict to a single target (`root`, `nce`, `ncd`, or `npd`). Defaults to all.
    #[arg(long)]
    pub crate_name: Option<String>,
}

struct CrateSpec {
    /// Short name used by `--crate-name` filter (e.g. `nce`).
    name: &'static str,
    /// Crate dir, relative to workspace root (e.g. `crates/riri-napi-nce`).
    crate_dir: &'static str,
    /// Bin script entrypoint, relative to crate dir.
    bin_script: &'static str,
    /// Fixture dir for example/debug runs, relative to workspace root.
    fixture_dir: &'static str,
    /// Tera template name registered for this crate's README.
    template: &'static str,
    /// Curated capability highlights for the root README tools section.
    features: &'static [&'static str],
    /// Serve packuments from `{fixture_dir}/registry` via a `file://` registry
    /// so example/debug output is deterministic and offline (ncd only).
    offline_registry: bool,
}

const CRATES: &[CrateSpec] = &[
    CrateSpec {
        name: "nce",
        crate_dir: "crates/riri-napi-nce",
        bin_script: "bin/nce.js",
        fixture_dir: "fixtures/nce-policy-supported-eol-bump",
        template: "readme-nce",
        features: &[
            "npm / pnpm / yarn lockfiles",
            "Node.js lifecycle & EOL policy gates",
            "multiple engine keys (node, npm, yarn)",
            "configurable version precision",
            "JSON output",
        ],
        offline_registry: false,
    },
    CrateSpec {
        name: "ncd",
        crate_dir: "crates/riri-napi-ncd",
        bin_script: "bin/ncd.js",
        fixture_dir: "fixtures/ncd-npm-deprecated-demo",
        template: "readme-ncd",
        features: &[
            "npm / yarn / pnpm lockfiles (auto-detected)",
            "dependency chains to each deprecated package",
            "semver-range blocker analysis",
            "newest non-deprecated version hints",
            "JSON output",
        ],
        offline_registry: true,
    },
    CrateSpec {
        name: "npd",
        crate_dir: "crates/riri-napi-npd",
        bin_script: "bin/npd.js",
        fixture_dir: "fixtures/npd-npm-v3-unpinned-deps",
        template: "readme-npd",
        features: &[
            "npm / yarn / pnpm lockfiles (auto-detected)",
            "workspace mode",
            "pnpm catalog pinning",
            "save-exact via .npmrc",
            "JSON output",
        ],
        offline_registry: false,
    },
];

pub fn run(args: &Args) -> anyhow::Result<()> {
    let workspace_root = workspace_root();
    let tera = build_tera()?;
    let mut drift = false;

    let do_root = matches!(args.crate_name.as_deref(), None | Some("root"));
    if do_root {
        let regenerated = regenerate_root(&workspace_root, &tera)?;
        emit(
            &workspace_root.join("README.md"),
            &regenerated,
            args.check,
            &mut drift,
        )?;
    }

    let selected: Vec<&CrateSpec> = match args.crate_name.as_deref() {
        Some("root") => Vec::new(),
        Some(name) => vec![
            CRATES
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| anyhow!("unknown crate: {name}. Known: root, nce, ncd, npd"))?,
        ],
        None => CRATES.iter().collect(),
    };
    for spec in selected {
        let readme_path = workspace_root.join(spec.crate_dir).join("README.md");
        let regenerated = regenerate(&workspace_root, &tera, spec)?;
        emit(&readme_path, &regenerated, args.check, &mut drift)?;
    }

    if drift {
        bail!("README drift detected. Run `cargo xtask regen-readme` to update.");
    }
    Ok(())
}

fn regenerate(workspace_root: &Path, tera: &Tera, spec: &CrateSpec) -> anyhow::Result<String> {
    let crate_path = workspace_root.join(spec.crate_dir);
    let bin_path = crate_path.join(spec.bin_script);
    if !bin_path.exists() {
        bail!(
            "missing bin script: {}. Run `npm ci && npx napi build --release --platform` in {}.",
            bin_path.display(),
            spec.crate_dir,
        );
    }
    let pkg_path = crate_path.join("package.json");
    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&pkg_path)
            .with_context(|| format!("read {}", pkg_path.display()))?,
    )?;
    let node_engines = pkg
        .get("engines")
        .and_then(|e| e.get("node"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("engines.node missing in {}", pkg_path.display()))?
        .to_string();

    let targets: Vec<String> = pkg
        .get("napi")
        .and_then(|n| n.get("targets"))
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let fixture_path = workspace_root.join(spec.fixture_dir);
    // Offline crates point `--registry` at a `file://` packument fixture so the
    // example/debug output is deterministic (no live registry).
    let registry_arg = spec
        .offline_registry
        .then(|| format!("file://{}", fixture_path.join("registry").display()));
    let mut example_args: Vec<&str> = Vec::new();
    let mut debug_args: Vec<&str> = vec!["-d"];
    if let Some(reg) = &registry_arg {
        example_args.extend(["--registry", reg.as_str()]);
        debug_args.extend(["--registry", reg.as_str()]);
    }

    let help = run_node(&bin_path, &["--help"], &crate_path)?;
    let example = run_node(&bin_path, &example_args, &fixture_path)?;
    let debug = run_node(&bin_path, &debug_args, &fixture_path)?;

    let mut ctx = TeraContext::new();
    ctx.insert("node_engines", &node_engines);
    ctx.insert(
        "platforms_table",
        &markdown_table(&["OS", "Arch"], &platform_rows(&targets)),
    );
    ctx.insert("help", &strip_trailing_ws(&help));
    ctx.insert("example", &strip_trailing_ws(&example));
    ctx.insert("debug", &strip_trailing_ws(&debug));

    let rendered = tera
        .render(spec.template, &ctx)
        .with_context(|| format!("render tera template `{}`", spec.template))?;
    Ok(rendered)
}

fn build_tera() -> anyhow::Result<Tera> {
    let mut tera = Tera::default();
    tera.autoescape_on(Vec::<&str>::new());
    tera.add_raw_templates(vec![
        ("readme-nce", TPL_README_NCE),
        ("readme-ncd", TPL_README_NCD),
        ("readme-npd", TPL_README_NPD),
        ("readme-root", TPL_README_ROOT),
    ])
    .context("register tera templates")?;
    Ok(tera)
}

fn run_node(bin: &Path, args: &[&str], cwd: &Path) -> anyhow::Result<String> {
    let mut cmd = Command::new("node");
    cmd.arg(bin).args(args).current_dir(cwd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = cmd
        .output()
        .with_context(|| format!("spawn node {} in {}", bin.display(), cwd.display()))?;
    // CLI writes via Rust to host process; capture both streams. `--help` and
    // run output both go to stdout; debug-mode spinner lines go to stderr.
    let mut combined = String::new();
    if !output.stderr.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    if !output.stdout.is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    Ok(combined)
}

fn strip_trailing_ws(input: &str) -> String {
    let trimmed = input.trim_end();
    let mut out = String::with_capacity(trimmed.len());
    for line in trimmed.split('\n') {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.pop();
    out
}

fn print_diff(original: &str, regenerated: &str) {
    let diff = TextDiff::from_lines(original, regenerated);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => continue,
        };
        print!("{sign}{change}");
    }
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .expect("xtask manifest under workspace")
        .to_path_buf()
}

const RUSTC_URL: &str = "https://www.rust-lang.org/tools/install";
const PREK_URL: &str = "https://prek.j178.dev/";
const NODE_URL: &str = "https://nodejs.org/";

/// Run `cmd args` and parse a version from combined stdout/stderr; `"n/a"` on failure.
fn tool_version(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().ok().map_or_else(
        || "n/a".to_string(),
        |o| {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            parse_version(&s)
        },
    )
}

/// Read `engines.node` shared by all published crates; error if they disagree or none exist.
fn node_constraint(workspace_root: &Path) -> anyhow::Result<String> {
    let mut found: Option<String> = None;
    for spec in CRATES {
        let pkg_path = workspace_root.join(spec.crate_dir).join("package.json");
        let pkg: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&pkg_path)
                .with_context(|| format!("read {}", pkg_path.display()))?,
        )?;
        let constraint = pkg
            .get("engines")
            .and_then(|e| e.get("node"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("engines.node missing in {}", pkg_path.display()))?
            .to_string();
        match &found {
            Some(prev) if *prev != constraint => {
                bail!("engines.node mismatch across crates: {prev:?} vs {constraint:?}")
            }
            _ => found = Some(constraint),
        }
    }
    found.ok_or_else(|| anyhow!("no crates to read engines.node from"))
}

/// One end-user tool entry for the Tools table.
fn tool_entry(workspace_root: &Path, spec: &CrateSpec) -> anyhow::Result<serde_json::Value> {
    let pkg_path = workspace_root.join(spec.crate_dir).join("package.json");
    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&pkg_path)
            .with_context(|| format!("read {}", pkg_path.display()))?,
    )?;
    let name = pkg
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("name missing in {}", pkg_path.display()))?;
    let description = pkg
        .get("description")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("description missing in {}", pkg_path.display()))?;
    let mut bins: Vec<String> = pkg
        .get("bin")
        .and_then(serde_json::Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    bins.sort();
    Ok(serde_json::json!({
        "npm_name": name,
        "npm_url": format!("https://npmx.dev/{name}"),
        "bins": bins,
        "description": description,
        "install": format!("npm i -g {name}"),
        "readme": format!("{}/README.md", spec.crate_dir),
        "features": spec.features,
    }))
}

/// Render a left-aligned GFM table (columns padded to their widest cell). Output
/// matches oxfmt's markdown table formatting so the generated README stays clean
/// under the repo's oxfmt pre-commit hook (no post-generation drift).
fn markdown_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    // Floor each column at 3 to match GFM's minimum separator width (`---`), which
    // is what oxfmt enforces; otherwise narrow columns would drift under the hook.
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count().max(3)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        let padded: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{c:<width$}", width = widths[i]))
            .collect();
        format!("| {} |", padded.join(" | "))
    };
    let header_cells: Vec<String> = headers.iter().map(|h| (*h).to_string()).collect();
    let sep_cells: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    let mut out = vec![line(&header_cells), line(&sep_cells)];
    out.extend(rows.iter().map(|r| line(r)));
    out.join("\n")
}

/// Build the Tools table rows from the tool entries produced by [`tool_entry`].
fn tools_rows(tools: &serde_json::Value) -> Vec<Vec<String>> {
    let str_of = |t: &serde_json::Value, k: &str| {
        t.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    tools
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|t| {
                    let bins = t
                        .get("bins")
                        .and_then(serde_json::Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(serde_json::Value::as_str)
                                .map(|b| format!("`{b}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    vec![
                        format!(
                            "[{}]({}) ({bins}) — [README]({})",
                            str_of(t, "npm_name"),
                            str_of(t, "npm_url"),
                            str_of(t, "readme")
                        ),
                        str_of(t, "description"),
                        format!("`{}`", str_of(t, "install")),
                    ]
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One bullet per tool: `- **<short name>** — feat · feat · …`, from the registry features.
fn tools_features(tools: &serde_json::Value) -> String {
    tools
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|t| {
                    let name = t
                        .get("npm_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let short = name.rsplit('/').next().unwrap_or(name);
                    let feats = t
                        .get("features")
                        .and_then(serde_json::Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(serde_json::Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" · ")
                        })
                        .unwrap_or_default();
                    format!("- **{short}** — {feats}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Read the napi build targets shared by all published crates; error if they disagree.
fn napi_targets(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut found: Option<Vec<String>> = None;
    for spec in CRATES {
        let pkg_path = workspace_root.join(spec.crate_dir).join("package.json");
        let pkg: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&pkg_path)
                .with_context(|| format!("read {}", pkg_path.display()))?,
        )?;
        let targets: Vec<String> = pkg
            .get("napi")
            .and_then(|n| n.get("targets"))
            .and_then(serde_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .ok_or_else(|| anyhow!("napi.targets missing in {}", pkg_path.display()))?;
        match &found {
            Some(prev) if *prev != targets => {
                bail!("napi.targets mismatch across crates: {prev:?} vs {targets:?}")
            }
            _ => found = Some(targets),
        }
    }
    found.ok_or_else(|| anyhow!("no crates to read napi.targets from"))
}

/// Map a Rust target triple to (display OS, display arch, optional libc). `None` for unknown.
fn target_os_arch_libc(triple: &str) -> Option<(&'static str, &'static str, Option<&'static str>)> {
    let arch = if triple.starts_with("x86_64") {
        "x64"
    } else if triple.starts_with("aarch64") {
        "arm64"
    } else {
        return None;
    };
    if triple.contains("linux") {
        Some((
            "Linux",
            arch,
            Some(if triple.ends_with("musl") {
                "musl"
            } else {
                "glibc"
            }),
        ))
    } else if triple.contains("darwin") {
        Some(("macOS", arch, None))
    } else if triple.contains("windows") {
        Some(("Windows", arch, None))
    } else {
        None
    }
}

/// Group target triples into `[OS, Architectures]` rows (libc noted in parens for Linux).
fn platform_rows(targets: &[String]) -> Vec<Vec<String>> {
    const OS_ORDER: [&str; 3] = ["Linux", "macOS", "Windows"];
    const ARCH_ORDER: [&str; 2] = ["x64", "arm64"];
    let parsed: Vec<_> = targets
        .iter()
        .filter_map(|t| target_os_arch_libc(t))
        .collect();
    let mut rows = Vec::new();
    for os in OS_ORDER {
        let mut arch_cells = Vec::new();
        for arch in ARCH_ORDER {
            if !parsed.iter().any(|(o, a, _)| *o == os && *a == arch) {
                continue;
            }
            let mut libcs: Vec<&str> = parsed
                .iter()
                .filter(|(o, a, _)| *o == os && *a == arch)
                .filter_map(|(_, _, l)| *l)
                .collect();
            libcs.dedup();
            arch_cells.push(if libcs.is_empty() {
                arch.to_string()
            } else {
                format!("{arch} ({})", libcs.join(", "))
            });
        }
        if !arch_cells.is_empty() {
            rows.push(vec![os.to_string(), arch_cells.join(", ")]);
        }
    }
    rows
}

fn render_root(
    tera: &Tera,
    prereqs: &serde_json::Value,
    tools: &serde_json::Value,
    targets: &[String],
) -> anyhow::Result<String> {
    let mut ctx = TeraContext::new();
    ctx.insert("prereqs", prereqs);
    ctx.insert(
        "tools_table",
        &markdown_table(&["Tool", "Description", "Install"], &tools_rows(tools)),
    );
    ctx.insert("tools_features", &tools_features(tools));
    ctx.insert(
        "platforms_table",
        &markdown_table(&["OS", "Architectures"], &platform_rows(targets)),
    );
    let rendered = tera
        .render("readme-root", &ctx)
        .context("render tera template `readme-root`")?;
    // Per-line trim + exactly one trailing newline, matching prek's end-of-file
    // and trailing-whitespace hooks so the committed file never drifts against them.
    Ok(format!("{}\n", strip_trailing_ws(&rendered)))
}

fn regenerate_root(workspace_root: &Path, tera: &Tera) -> anyhow::Result<String> {
    let node_constraint = node_constraint(workspace_root)?;
    let prereqs = serde_json::json!([
        {"name":"rustc","url":RUSTC_URL,"constraint":">=1.85.0 <2.0.0","tested_with":tool_version("rustc",&["--version"])},
        {"name":"prek","url":PREK_URL,"constraint":">=0.3.8","tested_with":tool_version("prek",&["--version"])},
        {"name":"node","url":NODE_URL,"constraint":node_constraint,"tested_with":tool_version("node",&["--version"])},
    ]);
    let tools = CRATES
        .iter()
        .map(|spec| tool_entry(workspace_root, spec))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let targets = napi_targets(workspace_root)?;
    render_root(tera, &prereqs, &serde_json::Value::Array(tools), &targets)
}

/// Compare `regenerated` against the file at `path`; write it, or (in check mode) record drift + print a diff.
fn emit(path: &Path, regenerated: &str, check: bool, drift: &mut bool) -> anyhow::Result<()> {
    let original =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if regenerated == original {
        println!("unchanged: {}", path.display());
    } else if check {
        *drift = true;
        println!("drift: {}", path.display());
        print_diff(&original, regenerated);
    } else {
        std::fs::write(path, regenerated).with_context(|| format!("write {}", path.display()))?;
        println!("wrote: {}", path.display());
    }
    Ok(())
}

/// Extract the first `MAJOR.MINOR.PATCH` triple from a `--version` line.
/// Returns `"n/a"` when none is present.
fn parse_version(raw: &str) -> String {
    raw.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|token| {
            let mut parts = token.split('.');
            let ok = |p: Option<&str>| {
                p.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            };
            ok(parts.next()) && ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
        })
        .unwrap_or("n/a")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_tera, parse_version, render_root};

    #[test]
    fn parses_rustc() {
        assert_eq!(
            parse_version("rustc 1.95.0 (e408947bf 2026-03-25)"),
            "1.95.0"
        );
    }

    #[test]
    fn parses_prek() {
        assert_eq!(parse_version("prek 0.4.3 (Homebrew 2026-06-04)"), "0.4.3");
    }

    #[test]
    fn parses_node() {
        assert_eq!(parse_version("v22.22.2"), "22.22.2");
    }

    #[test]
    fn falls_back_when_absent() {
        assert_eq!(parse_version("command not found"), "n/a");
    }

    #[test]
    fn renders_tools_table_and_prereqs() {
        let tera = build_tera().expect("build_tera failed");
        let prereqs = serde_json::json!([
            {"name":"rustc","url":"u","constraint":">=1.85.0 <2.0.0","tested_with":"1.95.0"},
            {"name":"node","url":"n","constraint":"^22.22.2 || ^24.15.0 || >=26","tested_with":"22.22.2"}
        ]);
        let tools = serde_json::json!([
            {"npm_name":"@smarlhens/npm-check-engines","npm_url":"https://npmx.dev/@smarlhens/npm-check-engines","bins":["nce","npm-check-engines"],"description":"Check & update Node.js engine constraints in package.json","install":"npm i -g @smarlhens/npm-check-engines","readme":"crates/riri-napi-nce/README.md","features":["npm / pnpm / yarn lockfiles","JSON output"]}
        ]);
        let targets = vec![
            "x86_64-unknown-linux-gnu".to_string(),
            "aarch64-apple-darwin".to_string(),
        ];
        let out = render_root(&tera, &prereqs, &tools, &targets).expect("render_root failed");
        assert!(out.contains("[@smarlhens/npm-check-engines](https://npmx.dev/@smarlhens/npm-check-engines) (`nce`, `npm-check-engines`) — [README](crates/riri-napi-nce/README.md)"));
        assert!(out.contains("`npm i -g @smarlhens/npm-check-engines`"));
        assert!(
            out.contains("- **npm-check-engines** — npm / pnpm / yarn lockfiles · JSON output")
        );
        assert!(out.contains("- [rustc](u) **>=1.85.0 <2.0.0** (_tested with 1.95.0_)"));
        assert!(
            out.contains("- [node](n) **^22.22.2 || ^24.15.0 || >=26** (_tested with 22.22.2_)")
        );
        assert!(out.contains("## Table of Contents"));
        assert!(out.contains("prek install"));
        assert!(out.ends_with('\n') && !out.ends_with("\n\n"));
    }

    #[test]
    fn platform_rows_groups_by_os_with_libc() {
        let targets: Vec<String> = [
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        assert_eq!(
            super::platform_rows(&targets),
            vec![
                vec![
                    "Linux".to_string(),
                    "x64 (glibc, musl), arm64 (glibc, musl)".to_string()
                ],
                vec!["macOS".to_string(), "x64, arm64".to_string()],
                vec!["Windows".to_string(), "x64".to_string()],
            ]
        );
    }

    #[test]
    fn markdown_table_pads_columns_to_widest_cell() {
        let table = super::markdown_table(
            &["A", "Bee"],
            &[
                vec!["xx".to_string(), "y".to_string()],
                vec!["z".to_string(), "wwww".to_string()],
            ],
        );
        let expected = "| A   | Bee  |\n| --- | ---- |\n| xx  | y    |\n| z   | wwww |";
        assert_eq!(table, expected);
    }
}
