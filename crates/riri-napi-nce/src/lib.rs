#![deny(clippy::all)]
// NAPI exports are consumed by the C ABI, not Rust — dead_code is a false positive for cdylib
#![allow(dead_code)]
// NAPI #[napi] functions require owned String parameters, not &str
#![allow(clippy::needless_pass_by_value)]

mod check_engines;
mod semver;

use napi_derive::napi;

/// Run the `nce` CLI in-process. `argv` must include the program name at
/// index 0 (e.g. `["nce", "--json"]`). Returns the exit code.
#[napi]
#[must_use]
pub fn run_cli(argv: Vec<String>) -> i32 {
    riri_nce::cli::run_cli(argv)
}
