#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use clap::{Parser, Subcommand};
use xtask::{refresh_node, regen_readme};

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Workspace-internal tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Refresh the bundled Node.js lifecycle data file.
    RefreshNodeData(refresh_node::Args),
    /// Regenerate dynamic sections of napi-crate READMEs.
    RegenReadme(regen_readme::Args),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::RefreshNodeData(args) => refresh_node::run(&args),
        Cmd::RegenReadme(args) => regen_readme::run(&args),
    }
}
