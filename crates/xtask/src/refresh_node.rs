//! Refresh the bundled Node.js lifecycle data.

use anyhow::Context;
use riri_node_lifecycle::refresh::{AggregatedData, aggregate_offline, data_changed, fetch_remote};
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Output path. Defaults to the bundled file in `riri-node-lifecycle`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Read pre-fetched JSON files from this directory instead of the network.
    /// Expected files: `index.json`, `schedule.json`.
    #[arg(long)]
    pub offline_from: Option<PathBuf>,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let aggregated = if let Some(dir) = &args.offline_from {
        let index_raw = std::fs::read_to_string(dir.join("index.json"))
            .with_context(|| format!("read {}/index.json", dir.display()))?;
        let schedule_raw = std::fs::read_to_string(dir.join("schedule.json"))
            .with_context(|| format!("read {}/schedule.json", dir.display()))?;
        aggregate_offline(&index_raw, &schedule_raw)?
    } else {
        fetch_remote()?
    };
    let output_path = args.out.clone().unwrap_or_else(default_out_path);
    write_if_changed(&aggregated, &output_path)
}

fn write_if_changed(
    aggregated: &AggregatedData,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = std::fs::read_to_string(output_path).ok();
    if !data_changed(aggregated, existing.as_deref())? {
        println!("no data changes, skipping write {}", output_path.display());
        return Ok(());
    }
    let json = serde_json::to_string_pretty(aggregated)? + "\n";
    std::fs::write(output_path, json)?;
    println!("wrote {}", output_path.display());
    Ok(())
}

fn default_out_path() -> PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("xtask manifest under workspace")
        .join("crates/riri-node-lifecycle/data/node-versions.json")
}
