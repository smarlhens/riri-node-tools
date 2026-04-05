//! Core logic for `npm-check-engines`.
//!
//! Computes the most restrictive engine constraints from a lockfile's
//! dependency entries and compares them against the project's `package.json`.

mod compute;

pub use compute::{
    CheckEnginesInput, CheckEnginesOutput, EngineRangeToSet, check_engines,
    compute_engines_constraint, get_constraint_from_engines,
};
