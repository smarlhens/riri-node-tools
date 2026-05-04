//! Core logic for `npm-check-engines`.
//!
//! Computes the most restrictive engine constraints from a lockfile's
//! dependency entries and compares them against the project's `package.json`.

mod compute;
mod mutate;
mod npm_bump;
mod policy;

pub use compute::{
    CheckEnginesInput, CheckEnginesOutput, EngineRangeToSet, check_engines,
    compute_engines_constraint, get_constraint_from_engines,
};
pub use mutate::{apply_engines_to_lockfile, apply_engines_update};
pub use npm_bump::{NpmBumpError, NpmBumpResult, derive_npm_floor, maybe_bump_npm};
pub use policy::{EolWarning, PolicyContext, PolicyResult, RewriteError, rewrite_node_range};
