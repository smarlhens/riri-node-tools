use crate::ParsedRange;
use crate::helpers::apply_min_version;
use crate::subset::{intersects, is_subset_of};
use semver::Version;

/// Compute the most restrictive intersection of two ranges.
///
/// `r1` is the base (current most restrictive range). `r2` is the new constraint.
/// When there's no intersection, `r1` wins.
#[must_use]
pub fn restrictive_range(r1: &ParsedRange, r2: &ParsedRange) -> ParsedRange {
    // Empty ranges: return the other
    if r1.is_empty() {
        return r2.clone();
    }
    if r2.is_empty() {
        return r1.clone();
    }

    // Stage 1: Quick exits — subset check
    if is_subset_of(r1, r2) {
        return r1.clone();
    }
    if is_subset_of(r2, r1) {
        return r2.clone();
    }

    // Stage 2: Quick exit — no intersection
    if !intersects(r1, r2) {
        return r1.clone();
    }

    // Stage 3: Align major versions — only drop parts that are entirely
    // below the other side's minimum major. Open-ended parts and parts
    // whose upper bound reaches the target major must not be dropped.
    let (mut a, mut b) = (r1.clone(), r2.clone());
    while !a.is_empty() && !b.is_empty() && a.min_major() != b.min_major() {
        if a.min_major() < b.min_major() {
            if entirely_below(&a.parts[0], b.min_major()) {
                a = a.drop_first();
            } else {
                break;
            }
        } else if entirely_below(&b.parts[0], a.min_major()) {
            b = b.drop_first();
        } else {
            break;
        }
    }

    if a.is_empty() || b.is_empty() {
        return r1.clone();
    }

    // Stage 4: Synchronize min versions within same major
    let a_min = a.min_version().clone();
    let b_min = b.min_version().clone();

    if a_min != b_min {
        let higher_min = std::cmp::max(&a_min, &b_min).clone();
        a = apply_min_version(&a, &higher_min);
        b = apply_min_version(&b, &higher_min);

        if a.is_empty() || b.is_empty() {
            return r1.clone();
        }

        // After applying min, check again
        if !intersects(&a, &b) {
            return r1.clone();
        }

        // Recurse with aligned ranges
        return restrictive_range(&a, &b);
    }

    // Stage 5: Consume matching parts, recurse on remainder
    // Both start at the same min version — take the more restrictive first part
    let a_first = &a.parts[0];
    let b_first = &b.parts[0];

    // Keep the tighter (smaller upper bound) part
    let kept = pick_tighter(a_first, b_first);
    let a_rest = a.drop_first();
    let b_rest = b.drop_first();

    if a_rest.is_empty() && b_rest.is_empty() {
        return ParsedRange { parts: vec![kept] };
    }

    let remainder = if a_rest.is_empty() {
        b_rest
    } else if b_rest.is_empty() {
        a_rest
    } else {
        restrictive_range(&a_rest, &b_rest)
    };

    let mut parts = vec![kept];
    parts.extend(remainder.parts);
    ParsedRange { parts }
}

/// Check if a part's range is entirely below `target_major` (covers no
/// versions at that major).
fn entirely_below(part: &crate::RangePart, target_major: u64) -> bool {
    let target_min = Version::new(target_major, 0, 0);
    match (&part.max, &part.max_op) {
        (Some(max), Some(crate::Op::Lt)) => *max <= target_min,
        (Some(max), Some(crate::Op::Lte)) => *max < target_min,
        _ => false, // open-ended or no upper bound → never entirely below
    }
}

/// Pick the tighter of two range parts (the one with the smaller interval).
fn pick_tighter(a: &crate::RangePart, b: &crate::RangePart) -> crate::RangePart {
    // Compare upper bounds: smaller upper = tighter
    match (&a.max, &b.max) {
        (None, _) => b.clone(),
        (_, None) => a.clone(),
        (Some(a_max), Some(b_max)) => {
            if a_max <= b_max {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}
