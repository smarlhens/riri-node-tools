use crate::{Op, RangePart};
use semver::Version;
use smallvec::{SmallVec, smallvec};

/// A small, mostly-inline list of range parts (the common case is a single part
/// that does not span majors, kept on the stack).
pub type SplitParts = SmallVec<[RangePart; 1]>;

/// Split a cross-major range part into one part per major version.
///
/// e.g., `>=16.10.0 <18.0.0` → `[^16.10.0, ^17.0.0]`
/// e.g., `>=0.0.0 <=2.0.0` → `[^0.0.0, ^1.0.0, 2.0.0]`
#[must_use]
pub fn split_by_major(part: &RangePart) -> SplitParts {
    let Some(max) = &part.max else {
        return smallvec![part.clone()];
    };

    // Compute the effective exclusive upper major boundary
    let end_major = match part.max_op {
        Some(Op::Lt) => max.major,
        Some(Op::Lte) => max.major + 1,
        _ => return smallvec![part.clone()],
    };

    // Only split if the part spans more than one major
    if end_major <= part.min.major + 1 {
        return smallvec![part.clone()];
    }

    let mut parts: SplitParts = SmallVec::new();
    for major in part.min.major..end_major {
        let min = if major == part.min.major {
            part.min.clone()
        } else {
            Version::new(major, 0, 0)
        };

        // Last part: if the original has Lte, keep original bound
        let is_last = major == end_major - 1;
        if is_last && part.max_op == Some(Op::Lte) {
            parts.push(RangePart {
                min,
                min_op: Op::Gte,
                max: Some(max.clone()),
                max_op: Some(Op::Lte),
            });
        } else {
            parts.push(RangePart {
                min,
                min_op: Op::Gte,
                max: Some(Version::new(major + 1, 0, 0)),
                max_op: Some(Op::Lt),
            });
        }
    }
    parts
}
