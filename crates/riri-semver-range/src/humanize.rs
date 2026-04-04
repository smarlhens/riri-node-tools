use crate::helpers::split_by_major;
use crate::{Op, ParsedRange, RangePart};

impl ParsedRange {
    /// Convert the range to a human-readable string.
    ///
    /// Cross-major parts are split into one per major, then each part
    /// is formatted as caret (`^X.Y.Z`), gte (`>=X.Y.Z`), or raw bounds.
    #[must_use]
    pub fn humanize(&self) -> String {
        if self.parts.is_empty() {
            return "*".to_string();
        }

        // Check for wildcard (>=0.0.0, no upper bound)
        if self.parts.len() == 1
            && self.parts[0].max.is_none()
            && self.parts[0].min.major == 0
            && self.parts[0].min.minor == 0
            && self.parts[0].min.patch == 0
        {
            return "*".to_string();
        }

        let humanized: Vec<String> = self
            .parts
            .iter()
            .flat_map(split_by_major)
            .map(|p| humanize_part(&p))
            .collect();

        humanized.join(" || ")
    }
}

fn humanize_part(part: &RangePart) -> String {
    // Caret pattern: >=X.Y.Z <(X+1).0.0
    if part.is_caret() {
        return format!("^{}", part.min);
    }

    // Open-ended: >=X.Y.Z (no upper bound)
    if part.max.is_none() {
        return match part.min_op {
            Op::Gte => format!(">={}", part.min),
            Op::Gt => format!(">{}", part.min),
            _ => format!("{}", part.min),
        };
    }

    // Exact version: >=X.Y.Z <X.Y.(Z+1) or >=X.Y.Z <=X.Y.Z
    if let Some(max) = &part.max
        && part.min_op == Op::Gte
        && ((part.max_op == Some(Op::Lt)
            && max.major == part.min.major
            && max.minor == part.min.minor
            && max.patch == part.min.patch + 1)
            || (part.max_op == Some(Op::Lte) && *max == part.min))
    {
        return format!("{}", part.min);
    }

    // Generic bounded range
    let min_str = match part.min_op {
        Op::Gte => format!(">={}", part.min),
        Op::Gt => format!(">{}", part.min),
        _ => format!("{}", part.min),
    };

    match (&part.max, &part.max_op) {
        (Some(max), Some(Op::Lt)) => format!("{min_str} <{max}"),
        (Some(max), Some(Op::Lte)) => format!("{min_str} <={max}"),
        _ => min_str,
    }
}
