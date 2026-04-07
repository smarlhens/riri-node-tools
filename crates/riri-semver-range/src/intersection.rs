use crate::helpers::split_by_major;
use crate::subset::{intersects, is_subset_of};
use crate::{Op, ParsedRange, RangePart};

/// Compute the most restrictive intersection of two ranges.
///
/// `r1` is the base (current most restrictive range). `r2` is the new constraint.
/// When there's no intersection, `r1` wins.
///
/// Both ranges are split into one-part-per-major, then every pair of parts
/// is intersected. Non-empty intersections form the result.
#[must_use]
pub fn restrictive_range(r1: &ParsedRange, r2: &ParsedRange) -> ParsedRange {
    if r1.is_empty() {
        return r2.clone();
    }
    if r2.is_empty() {
        return r1.clone();
    }

    if is_subset_of(r1, r2) {
        return r1.clone();
    }
    if is_subset_of(r2, r1) {
        return r2.clone();
    }

    if !intersects(r1, r2) {
        return r1.clone();
    }

    // Split both ranges into one part per major version
    let a_parts: Vec<_> = r1.parts.iter().flat_map(split_by_major).collect();
    let b_parts: Vec<_> = r2.parts.iter().flat_map(split_by_major).collect();

    // Pairwise interval intersection
    let mut result_parts = Vec::new();
    for a_part in &a_parts {
        for b_part in &b_parts {
            if let Some(intersection) = intersect_parts(a_part, b_part) {
                result_parts.push(intersection);
            }
        }
    }

    if result_parts.is_empty() {
        return r1.clone();
    }

    ParsedRange {
        parts: result_parts,
    }
}

/// Compute the interval intersection of two range parts.
///
/// Returns `None` if the intersection is empty.
fn intersect_parts(a: &RangePart, b: &RangePart) -> Option<RangePart> {
    // Higher min (more restrictive lower bound)
    let (min, min_op) = match a.min.cmp(&b.min) {
        std::cmp::Ordering::Greater => (a.min.clone(), a.min_op),
        std::cmp::Ordering::Less => (b.min.clone(), b.min_op),
        std::cmp::Ordering::Equal => {
            // Same min — Gt is more restrictive than Gte
            let op = if a.min_op == Op::Gt || b.min_op == Op::Gt {
                Op::Gt
            } else {
                Op::Gte
            };
            (a.min.clone(), op)
        }
    };

    // Lower max (more restrictive upper bound)
    let (max, max_op) = match (&a.max, &b.max) {
        (None, None) => (None, None),
        (None, Some(_)) => (b.max.clone(), b.max_op),
        (Some(_), None) => (a.max.clone(), a.max_op),
        (Some(a_max), Some(b_max)) => match a_max.cmp(b_max) {
            std::cmp::Ordering::Less => (Some(a_max.clone()), a.max_op),
            std::cmp::Ordering::Greater => (Some(b_max.clone()), b.max_op),
            std::cmp::Ordering::Equal => {
                // Same max — Lt is more restrictive than Lte
                let op = if a.max_op == Some(Op::Lt) || b.max_op == Some(Op::Lt) {
                    Some(Op::Lt)
                } else {
                    a.max_op
                };
                (Some(a_max.clone()), op)
            }
        },
    };

    // Check if the interval is non-empty
    if let Some(ref max_version) = max {
        let empty = match (min_op, max_op) {
            (Op::Gte, Some(Op::Lte)) => min > *max_version,
            (Op::Gte | Op::Gt, Some(Op::Lt | Op::Lte)) => min >= *max_version,
            _ => false,
        };
        if empty {
            return None;
        }
    }

    Some(RangePart {
        min,
        min_op,
        max,
        max_op,
    })
}
