//! Regenerate napi-crate READMEs from full Tera templates.

use anyhow::{Context, anyhow, bail};
use similar::TextDiff;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tera::{Context as TeraContext, Tera};

const TPL_README_NCE: &str = include_str!("../templates/readme-nce.tera");
const TPL_README_NPD: &str = include_str!("../templates/readme-npd.tera");

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Fail with a non-zero exit code and print a diff if the README would change.
    #[arg(long)]
    pub check: bool,
    /// Restrict to a single napi crate (`nce` or `npd`). Defaults to all.
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
}

const CRATES: &[CrateSpec] = &[
    CrateSpec {
        name: "nce",
        crate_dir: "crates/riri-napi-nce",
        bin_script: "bin/nce.js",
        fixture_dir: "fixtures/nce-policy-supported-eol-bump",
        template: "readme-nce",
    },
    CrateSpec {
        name: "npd",
        crate_dir: "crates/riri-napi-npd",
        bin_script: "bin/npd.js",
        fixture_dir: "fixtures/npd-npm-v3-unpinned-deps",
        template: "readme-npd",
    },
];

pub fn run(args: &Args) -> anyhow::Result<()> {
    let workspace_root = workspace_root();
    let tera = build_tera()?;
    let mut drift = false;
    let selected: Vec<&CrateSpec> = match &args.crate_name {
        Some(name) => {
            let spec = CRATES
                .iter()
                .find(|c| c.name == name)
                .ok_or_else(|| anyhow!("unknown crate: {name}. Known: nce, npd"))?;
            vec![spec]
        }
        None => CRATES.iter().collect(),
    };
    for spec in selected {
        let readme_path = workspace_root.join(spec.crate_dir).join("README.md");
        let original = std::fs::read_to_string(&readme_path)
            .with_context(|| format!("read {}", readme_path.display()))?;
        let regenerated = regenerate(&workspace_root, &tera, spec)?;
        if regenerated == original {
            println!("unchanged: {}", readme_path.display());
            continue;
        }
        if args.check {
            drift = true;
            println!("drift: {}", readme_path.display());
            print_diff(&original, &regenerated);
        } else {
            std::fs::write(&readme_path, &regenerated)
                .with_context(|| format!("write {}", readme_path.display()))?;
            println!("wrote: {}", readme_path.display());
        }
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

    let help = run_node(&bin_path, &["--help"], &crate_path)?;
    let fixture_path = workspace_root.join(spec.fixture_dir);
    let example = run_node(&bin_path, &[], &fixture_path)?;
    let debug = run_node(&bin_path, &["-d"], &fixture_path)?;

    let mut ctx = TeraContext::new();
    ctx.insert("node_engines", &node_engines);
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
    tera.autoescape_on(Vec::new());
    tera.add_raw_templates(vec![
        ("readme-nce", TPL_README_NCE),
        ("readme-npd", TPL_README_NPD),
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
