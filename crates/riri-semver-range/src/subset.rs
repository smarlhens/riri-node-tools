use crate::{Op, ParsedRange, RangePart};

/// Returns `true` if every version satisfying `r1` also satisfies `r2`.
///
/// For each part in `r1`, there must exist a part in `r2` that fully contains it.
#[must_use]
pub fn is_subset_of(r1: &ParsedRange, r2: &ParsedRange) -> bool {
    // An empty/impossible range is a subset of everything
    if r1.parts.iter().all(is_impossible) {
        return true;
    }
    r1.parts
        .iter()
        .all(|a| is_impossible(a) || r2.parts.iter().any(|b| part_is_subset(a, b)))
}

/// Returns `true` if there exists any version satisfying both ranges.
#[must_use]
pub fn intersects(r1: &ParsedRange, r2: &ParsedRange) -> bool {
    r1.parts
        .iter()
        .any(|a| r2.parts.iter().any(|b| parts_intersect(a, b)))
}

/// Check if part `a` is fully contained within part `b`.
fn part_is_subset(a: &RangePart, b: &RangePart) -> bool {
    // b's lower bound must be <= a's lower bound
    let b_lower_ok = match (&a.min_op, &b.min_op) {
        (Op::Gte | Op::Gt, Op::Gte) | (Op::Gt, Op::Gt) => b.min <= a.min,
        (Op::Gte, Op::Gt) => b.min < a.min,
        _ => false,
    };

    // b's upper bound must be >= a's upper bound
    let b_upper_ok = match (&a.max, &a.max_op, &b.max, &b.max_op) {
        (_, _, None, _) => true,
        (Some(a_max), Some(a_op), Some(b_max), Some(b_op)) => match (a_op, b_op) {
            (Op::Lt, Op::Lt | Op::Lte) | (Op::Lte, Op::Lte) => a_max <= b_max,
            (Op::Lte, Op::Lt) => a_max < b_max,
            _ => false,
        },
        _ => false,
    };

    b_lower_ok && b_upper_ok
}

/// Check if two range parts (intervals) overlap.
fn parts_intersect(a: &RangePart, b: &RangePart) -> bool {
    if is_impossible(a) || is_impossible(b) {
        return false;
    }

    let a_below_b_max = match (&b.max, &b.max_op) {
        (None, _) => true,
        (Some(max), Some(Op::Lt)) => &a.min < max,
        (Some(max), Some(Op::Lte)) => match a.min_op {
            Op::Gt => &a.min < max,
            _ => &a.min <= max,
        },
        _ => false,
    };

    let b_below_a_max = match (&a.max, &a.max_op) {
        (None, _) => true,
        (Some(max), Some(Op::Lt)) => &b.min < max,
        (Some(max), Some(Op::Lte)) => match b.min_op {
            Op::Gt => &b.min < max,
            _ => &b.min <= max,
        },
        _ => false,
    };

    a_below_b_max && b_below_a_max
}

/// Check if a range part is impossible (e.g., >2.0.0 <1.0.0).
fn is_impossible(part: &RangePart) -> bool {
    match (&part.max, &part.max_op) {
        (Some(max), Some(Op::Lt)) => &part.min >= max,
        (Some(max), Some(Op::Lte)) => match part.min_op {
            Op::Gt => &part.min >= max,
            _ => &part.min > max,
        },
        _ => false,
    }
}
