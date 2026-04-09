#![deny(clippy::all)]
// NAPI exports are consumed by the C ABI, not Rust — dead_code is a false positive for cdylib
#![allow(dead_code)]
// NAPI #[napi] functions require owned String parameters, not &str
#![allow(clippy::needless_pass_by_value)]

mod check_engines;
mod semver;
