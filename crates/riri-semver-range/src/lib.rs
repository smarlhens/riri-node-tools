pub(crate) mod helpers;
mod humanize;
mod intersection;
mod parse;
pub(crate) mod subset;

pub use intersection::restrictive_range;
pub use parse::ParsedRange;
pub use subset::{intersects, is_subset_of};

use semver::Version;

fn same_tuple(v: &Version, tuple: (u64, u64, u64)) -> bool {
    (v.major, v.minor, v.patch) == tuple
}

/// Comparison operator for a bound in a range part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Gte,
    Gt,
    Lt,
    Lte,
}

/// A single segment of a `||`-separated semver range.
///
/// Represents a contiguous version interval, e.g. `>=16.10.0 <17.0.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangePart {
    pub min: Version,
    pub min_op: Op,
    pub max: Option<Version>,
    pub max_op: Option<Op>,
}

impl RangePart {
    #[must_use]
    pub fn major(&self) -> u64 {
        self.min.major
    }

    /// Returns `true` if this part matches a caret pattern:
    /// - `>=X.Y.Z <(X+1).0.0` (standard caret, including `^0`)
    /// - `>=0.Y.Z <0.(Y+1).0` (zero-major caret, Y > 0)
    /// - `>=0.0.Z <0.0.(Z+1)` (zero-zero caret)
    #[must_use]
    pub fn is_caret(&self) -> bool {
        let (Op::Gte, Some(Op::Lt), Some(max)) = (&self.min_op, &self.max_op, &self.max) else {
            return false;
        };
        // Standard caret: >=X.Y.Z <(X+1).0.0 — only for major > 0.
        // For major 0, node-semver caret has special semantics (locks minor or patch).
        if self.min.major > 0 && max.major == self.min.major + 1 && max.minor == 0 && max.patch == 0
        {
            return true;
        }
        // Zero-major caret: >=0.Y.Z <0.(Y+1).0
        if self.min.major == 0
            && max.major == 0
            && max.minor == self.min.minor + 1
            && max.patch == 0
        {
            return true;
        }
        // Zero-zero caret: >=0.0.Z <0.0.(Z+1)
        self.min.major == 0
            && self.min.minor == 0
            && max.major == 0
            && max.minor == 0
            && max.patch == self.min.patch + 1
    }

    /// Returns `true` if the given version satisfies this range part.
    ///
    /// Applies node-semver prerelease filtering: a version with a prerelease
    /// tag is only allowed if a bound in this part has a prerelease on the
    /// same `[major, minor, patch]` tuple.
    #[must_use]
    pub fn satisfies(&self, version: &Version) -> bool {
        let min_ok = match self.min_op {
            Op::Gte => version >= &self.min,
            Op::Gt => version > &self.min,
            _ => false,
        };
        let max_ok = match (&self.max, &self.max_op) {
            (Some(max), Some(Op::Lt)) => version < max,
            (Some(max), Some(Op::Lte)) => version <= max,
            (None, None) => true,
            _ => false,
        };
        if !min_ok || !max_ok {
            return false;
        }
        // Node-semver prerelease filter: prerelease versions only match if
        // a bound explicitly has a prerelease on the same [major.minor.patch].
        if !version.pre.is_empty() {
            let tuple = (version.major, version.minor, version.patch);
            let min_allows = !self.min.pre.is_empty() && same_tuple(&self.min, tuple);
            let max_allows = self
                .max
                .as_ref()
                .is_some_and(|m| !m.pre.is_empty() && same_tuple(m, tuple));
            if !min_allows && !max_allows {
                return false;
            }
        }
        true
    }
}
