use crate::{Op, ParsedRange, RangePart};
use semver::Version;

/// Apply a minimum version to a range: drop parts below `min`'s major,
/// bump same-major parts whose min is below `min`.
#[must_use]
pub fn apply_min_version(range: &ParsedRange, min: &Version) -> ParsedRange {
    let parts: Vec<RangePart> = range
        .parts
        .iter()
        .filter_map(|part| {
            // Drop parts whose max is below min
            if let (Some(max), Some(Op::Lt)) = (&part.max, &part.max_op)
                && max <= min
            {
                return None;
            }
            if let (Some(max), Some(Op::Lte)) = (&part.max, &part.max_op)
                && max < min
            {
                return None;
            }

            // For same-major parts: bump lower bound if needed
            if part.min < *min {
                Some(RangePart {
                    min: min.clone(),
                    min_op: Op::Gte,
                    max: part.max.clone(),
                    max_op: part.max_op,
                })
            } else {
                Some(part.clone())
            }
        })
        .collect();

    ParsedRange { parts }
}

/// Split a cross-major range part into one part per major version.
///
/// e.g., `>=16.10.0 <18.0.0` → `[^16.10.0, ^17.0.0]`
/// e.g., `>=0.0.0 <=2.0.0` → `[^0.0.0, ^1.0.0, 2.0.0]`
#[must_use]
pub fn split_by_major(part: &RangePart) -> Vec<RangePart> {
    let Some(max) = &part.max else {
        return vec![part.clone()];
    };

    // Compute the effective exclusive upper major boundary
    let end_major = match part.max_op {
        Some(Op::Lt) => max.major,
        Some(Op::Lte) => max.major + 1,
        _ => return vec![part.clone()],
    };

    // Only split if the part spans more than one major
    if end_major <= part.min.major + 1 {
        return vec![part.clone()];
    }

    let mut parts = Vec::new();
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
