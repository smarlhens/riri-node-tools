use crate::{Op, RangePart};
use semver::Version;

/// A full semver range: multiple parts joined by `||`.
#[derive(Debug, Clone)]
pub struct ParsedRange {
    pub parts: Vec<RangePart>,
}

impl ParsedRange {
    /// Parse a semver range string like `"^14.17.0 || ^16.10.0 || >=17.0.0"`.
    ///
    /// # Errors
    ///
    /// Returns an error if any part of the range cannot be parsed.
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() || is_wildcard_str(input) {
            return Ok(Self {
                parts: vec![wildcard()],
            });
        }

        let mut parts: Vec<RangePart> = input
            .split("||")
            .map(|s| parse_comparator_set(s.trim()))
            .collect::<Result<Vec<_>, _>>()?;

        parts.sort_by(|a, b| a.min.cmp(&b.min));

        Ok(Self { parts })
    }

    /// Returns `true` if the range has no parts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Returns the minimum version across all parts.
    ///
    /// # Panics
    ///
    /// Panics if the range has no parts.
    #[must_use]
    pub fn min_version(&self) -> &Version {
        &self.parts[0].min
    }

    /// Returns the major version of the first (lowest) part.
    ///
    /// # Panics
    ///
    /// Panics if the range has no parts.
    #[must_use]
    pub fn min_major(&self) -> u64 {
        self.parts[0].major()
    }

    /// Returns a new range without the first part.
    #[must_use]
    pub fn drop_first(&self) -> Self {
        Self {
            parts: self.parts[1..].to_vec(),
        }
    }

    /// Returns `true` if the given version satisfies any part of this range.
    #[must_use]
    pub fn satisfies(&self, version: &Version) -> bool {
        self.parts.iter().any(|part| part.satisfies(version))
    }
}

fn wildcard() -> RangePart {
    RangePart {
        min: Version::new(0, 0, 0),
        min_op: Op::Gte,
        max: None,
        max_op: None,
    }
}

/// An impossible range that no version can satisfy (e.g., `>*`).
fn impossible() -> RangePart {
    RangePart {
        min: Version::new(0, 0, 0),
        min_op: Op::Gt,
        max: Some(Version::new(0, 0, 0)),
        max_op: Some(Op::Lt),
    }
}

/// Parse a single comparator set (one segment between `||`).
fn parse_comparator_set(input: &str) -> Result<RangePart, String> {
    let input = input.trim();

    if input.is_empty() || is_wildcard_str(input) {
        return Ok(wildcard());
    }

    // Hyphen range: "1.0.0 - 2.0.0"
    if let Some(idx) = input.find(" - ") {
        return parse_hyphen_range(&input[..idx], &input[idx + 3..]);
    }

    // Split into whitespace-separated tokens, merging bare operators with next token.
    // This handles ">= 1.0.0", "< 2.0.0", "~ 1.0", "^ 1.2", "~> 1", etc.
    let tokens = tokenize_comparator_set(input);

    if tokens.len() == 1 {
        return parse_single_comparator(&tokens[0]);
    }

    // Multi-comparator: intersect all (e.g., ">=1.0.0 <2.0.0" or "~1.2.1 >=1.2.3")
    // Parse each token as a comparator, then intersect by taking tightest bounds.
    let mut min = Version::new(0, 0, 0);
    let mut min_op = Op::Gte;
    let mut max: Option<Version> = None;
    let mut max_op: Option<Op> = None;

    for token in &tokens {
        let part = parse_single_comparator(token)?;

        // Take the higher lower bound
        if part.min > min || (part.min == min && part.min_op == Op::Gt && min_op == Op::Gte) {
            min = part.min;
            min_op = part.min_op;
        }

        // Take the lower upper bound
        if let (Some(p_max), Some(p_op)) = (&part.max, &part.max_op) {
            match &max {
                None => {
                    max = Some(p_max.clone());
                    max_op = Some(*p_op);
                }
                Some(cur_max) => {
                    if p_max < cur_max
                        || (p_max == cur_max && *p_op == Op::Lt && max_op == Some(Op::Lte))
                    {
                        max = Some(p_max.clone());
                        max_op = Some(*p_op);
                    }
                }
            }
        }
    }

    Ok(RangePart {
        min,
        min_op,
        max,
        max_op,
    })
}

/// Parse a single comparator token.
fn parse_single_comparator(token: &str) -> Result<RangePart, String> {
    let token = strip_v(token);
    // Strip build metadata early so it doesn't interfere with pattern detection
    let token = token.split('+').next().unwrap_or(token);

    if token.is_empty() || is_wildcard_str(token) {
        return Ok(wildcard());
    }

    // Caret: ^1.2.3 or ^x
    if let Some(rest) = token.strip_prefix('^') {
        let rest = strip_v(rest);
        if is_wildcard_str(rest) || rest.is_empty() {
            return Ok(wildcard());
        }
        return parse_caret(rest);
    }

    // Tilde: ~1.2.3 or ~>1.2.3
    if let Some(rest) = token.strip_prefix('~') {
        let rest = strip_v(rest);
        let rest = rest.strip_prefix('>').unwrap_or(rest);
        let rest = strip_v(rest);
        if is_wildcard_str(rest) || rest.is_empty() {
            return Ok(wildcard());
        }
        return parse_tilde(rest);
    }

    // Operators: >=, >, <=, <, =
    if let Some(rest) = token.strip_prefix(">=") {
        let rest = strip_v(rest);
        return parse_op_range(rest, true);
    }
    if let Some(rest) = token.strip_prefix('>') {
        let rest = strip_v(rest);
        return parse_gt_range(rest);
    }
    if let Some(rest) = token.strip_prefix("<=") {
        let rest = strip_v(rest);
        return parse_lte_range(rest);
    }
    if let Some(rest) = token.strip_prefix('<') {
        let rest = strip_v(rest);
        return parse_lt_range(rest);
    }
    if let Some(rest) = token.strip_prefix('=') {
        let rest = strip_v(rest);
        return parse_eq_range(rest);
    }

    // X-ranges: 1.2.x, 1.x, 1.2.*, 2, 2.3
    if is_x_range(token) {
        return parse_x_range(token);
    }

    // Bare version: treat as exact
    let v = parse_strict_version(token)?;
    Ok(RangePart {
        min: v.clone(),
        min_op: Op::Gte,
        max: Some(Version::new(v.major, v.minor, v.patch + 1)),
        max_op: Some(Op::Lt),
    })
}

/// Split a comparator set into tokens, merging bare operators with the
/// following version token.  e.g. `">= 1.0.0 < 2.0.0"` → `[">=1.0.0", "<2.0.0"]`.
fn tokenize_comparator_set(input: &str) -> Vec<String> {
    let raw: Vec<&str> = input.split_whitespace().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if is_bare_operator(raw[i]) && i + 1 < raw.len() {
            tokens.push(format!("{}{}", raw[i], raw[i + 1]));
            i += 2;
        } else {
            tokens.push(raw[i].to_string());
            i += 1;
        }
    }
    tokens
}

/// Check if a token is a bare operator with no version attached.
fn is_bare_operator(token: &str) -> bool {
    matches!(token, ">=" | ">" | "<=" | "<" | "=" | "~>" | "~" | "^")
}

fn strip_v(input: &str) -> &str {
    input.strip_prefix('v').unwrap_or(input).trim_start()
}

fn is_wildcard_str(s: &str) -> bool {
    matches!(s, "*" | "x" | "X")
}

fn is_x_range(token: &str) -> bool {
    token.contains('x')
        || token.contains('X')
        || token.contains('*')
        || token.split('.').count() < 3
}

/// `>=` with partial version support: `>=1.2` → `>=1.2.0`
fn parse_op_range(input: &str, _gte: bool) -> Result<RangePart, String> {
    if is_x_range(input) {
        let xr = parse_x_range(input)?;
        return Ok(RangePart {
            min: xr.min,
            min_op: Op::Gte,
            max: None,
            max_op: None,
        });
    }
    let v = parse_partial_version(input)?;
    Ok(RangePart {
        min: v,
        min_op: Op::Gte,
        max: None,
        max_op: None,
    })
}

fn parse_gt_range(input: &str) -> Result<RangePart, String> {
    if is_x_range(input) {
        let xr = parse_x_range(input)?;
        if xr.max.is_none() {
            // >* / >x means "greater than everything" → impossible range
            return Ok(impossible());
        }
        // >1.x means >=2.0.0
        return Ok(RangePart {
            min: xr.max.unwrap_or(xr.min),
            min_op: Op::Gte,
            max: None,
            max_op: None,
        });
    }
    let v = parse_partial_version(input)?;
    Ok(RangePart {
        min: v,
        min_op: Op::Gt,
        max: None,
        max_op: None,
    })
}

fn parse_lte_range(input: &str) -> Result<RangePart, String> {
    if is_x_range(input) {
        let xr = parse_x_range(input)?;
        // <=1.x means <2.0.0
        return Ok(RangePart {
            min: Version::new(0, 0, 0),
            min_op: Op::Gte,
            max: xr.max,
            max_op: xr.max_op.or(Some(Op::Lte)),
        });
    }
    let v = parse_partial_version(input)?;
    Ok(RangePart {
        min: Version::new(0, 0, 0),
        min_op: Op::Gte,
        max: Some(v),
        max_op: Some(Op::Lte),
    })
}

fn parse_lt_range(input: &str) -> Result<RangePart, String> {
    if is_x_range(input) {
        let xr = parse_x_range(input)?;
        // <1.x means <1.0.0
        return Ok(RangePart {
            min: Version::new(0, 0, 0),
            min_op: Op::Gte,
            max: Some(xr.min),
            max_op: Some(Op::Lt),
        });
    }
    let v = parse_partial_version(input)?;
    Ok(RangePart {
        min: Version::new(0, 0, 0),
        min_op: Op::Gte,
        max: Some(v),
        max_op: Some(Op::Lt),
    })
}

fn parse_eq_range(input: &str) -> Result<RangePart, String> {
    if is_x_range(input) {
        return parse_x_range(input);
    }
    let v = parse_partial_version(input)?;
    Ok(RangePart {
        min: v.clone(),
        min_op: Op::Gte,
        max: Some(Version::new(v.major, v.minor, v.patch + 1)),
        max_op: Some(Op::Lt),
    })
}

fn parse_caret(input: &str) -> Result<RangePart, String> {
    let parts_count = version_component_count(input);
    let v = parse_partial_version(input)?;

    let upper = match parts_count {
        // ^0 → <1.0.0, ^1 → <2.0.0 (only major specified)
        1 => Version::new(v.major + 1, 0, 0),
        2 => {
            if v.major == 0 {
                // ^0.1 → <0.2.0
                Version::new(0, v.minor + 1, 0)
            } else {
                // ^1.2 → <2.0.0
                Version::new(v.major + 1, 0, 0)
            }
        }
        _ => {
            // Full version: standard caret rules
            if v.major == 0 {
                if v.minor == 0 {
                    // ^0.0.1 → <0.0.2
                    Version::new(0, 0, v.patch + 1)
                } else {
                    // ^0.1.2 → <0.2.0
                    Version::new(0, v.minor + 1, 0)
                }
            } else {
                // ^1.2.3 → <2.0.0
                Version::new(v.major + 1, 0, 0)
            }
        }
    };
    Ok(RangePart {
        min: v,
        min_op: Op::Gte,
        max: Some(upper),
        max_op: Some(Op::Lt),
    })
}

fn parse_tilde(input: &str) -> Result<RangePart, String> {
    let parts_count = version_component_count(input);
    let v = parse_partial_version(input)?;
    let upper = if parts_count == 1 {
        // ~1 means >=1.0.0 <2.0.0
        Version::new(v.major + 1, 0, 0)
    } else {
        Version::new(v.major, v.minor + 1, 0)
    };
    Ok(RangePart {
        min: v,
        min_op: Op::Gte,
        max: Some(upper),
        max_op: Some(Op::Lt),
    })
}

fn parse_hyphen_range(low: &str, high: &str) -> Result<RangePart, String> {
    let low = strip_v(low.trim());
    let high = strip_v(high.trim());

    if is_wildcard_str(low) || low.is_empty() {
        let min = Version::new(0, 0, 0);
        if is_wildcard_str(high) || high.is_empty() {
            return Ok(wildcard());
        }
        let (max, max_op) = parse_hyphen_high(high)?;
        return Ok(RangePart {
            min,
            min_op: Op::Gte,
            max: Some(max),
            max_op: Some(max_op),
        });
    }

    // Low side may contain x: "1.x - x" → treat 1.x as >=1.0.0
    let min = if is_x_range(low) {
        let xr = parse_x_range(low)?;
        xr.min
    } else {
        parse_partial_version(low)?
    };
    if high == "x" || high == "*" || high.is_empty() {
        return Ok(RangePart {
            min,
            min_op: Op::Gte,
            max: None,
            max_op: None,
        });
    }

    let (max, max_op) = parse_hyphen_high(high)?;
    Ok(RangePart {
        min,
        min_op: Op::Gte,
        max: Some(max),
        max_op: Some(max_op),
    })
}

fn parse_hyphen_high(high: &str) -> Result<(Version, Op), String> {
    let parts: Vec<&str> = high.split('.').collect();
    // If it contains x/* or is partial, treat upper bound as exclusive next major/minor
    let has_x = parts.iter().any(|p| is_wildcard_str(p));
    if has_x || parts.len() < 3 {
        // Grab the numeric prefix
        let major = parse_u64(parts[0])?;
        if parts.len() == 1 || has_x && parts.len() <= 2 {
            Ok((Version::new(major + 1, 0, 0), Op::Lt))
        } else if let Ok(minor) = parse_u64(parts[1]) {
            Ok((Version::new(major, minor + 1, 0), Op::Lt))
        } else {
            Ok((Version::new(major + 1, 0, 0), Op::Lt))
        }
    } else {
        let v = parse_partial_version(high)?;
        Ok((v, Op::Lte))
    }
}

fn parse_x_range(input: &str) -> Result<RangePart, String> {
    let parts: Vec<&str> = input.split('.').collect();
    match parts.as_slice() {
        ["*" | "x" | "X"] => Ok(wildcard()),
        [major] => {
            let major = parse_u64(major)?;
            Ok(RangePart {
                min: Version::new(major, 0, 0),
                min_op: Op::Gte,
                max: Some(Version::new(major + 1, 0, 0)),
                max_op: Some(Op::Lt),
            })
        }
        [major, minor] if is_wildcard_str(minor) => {
            let major = parse_u64(major)?;
            Ok(RangePart {
                min: Version::new(major, 0, 0),
                min_op: Op::Gte,
                max: Some(Version::new(major + 1, 0, 0)),
                max_op: Some(Op::Lt),
            })
        }
        [major, minor] => {
            let major = parse_u64(major)?;
            let minor = parse_u64(minor)?;
            Ok(RangePart {
                min: Version::new(major, minor, 0),
                min_op: Op::Gte,
                max: Some(Version::new(major, minor + 1, 0)),
                max_op: Some(Op::Lt),
            })
        }
        [major, minor, patch] if is_wildcard_str(minor) && is_wildcard_str(patch) => {
            let major = parse_u64(major)?;
            Ok(RangePart {
                min: Version::new(major, 0, 0),
                min_op: Op::Gte,
                max: Some(Version::new(major + 1, 0, 0)),
                max_op: Some(Op::Lt),
            })
        }
        [major, minor, patch] if is_wildcard_str(patch) => {
            let major = parse_u64(major)?;
            let minor = parse_u64(minor)?;
            Ok(RangePart {
                min: Version::new(major, minor, 0),
                min_op: Op::Gte,
                max: Some(Version::new(major, minor + 1, 0)),
                max_op: Some(Op::Lt),
            })
        }
        _ => Err(format!("unsupported x-range: {input}")),
    }
}

fn parse_strict_version(input: &str) -> Result<Version, String> {
    Version::parse(input).map_err(|e| format!("invalid version '{input}': {e}"))
}

fn parse_partial_version(input: &str) -> Result<Version, String> {
    let input = strip_v(input);
    // Strip build metadata (+...)
    let input = input.split('+').next().unwrap_or(input);
    // Separate prerelease (-...) but preserve it
    let (base, pre) = if let Some(idx) = input.find('-') {
        (&input[..idx], Some(&input[idx..]))
    } else {
        (input, None)
    };
    // Filter out x/*/X wildcard components, treating them as absent
    let parts: Vec<&str> = base
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"))
        .collect();
    let version_str = match parts.as_slice() {
        [] => "0.0.0".to_string(),
        [major] => {
            parse_u64(major)?;
            format!("{major}.0.0")
        }
        [major, minor] => {
            parse_u64(major)?;
            parse_u64(minor)?;
            format!("{major}.{minor}.0")
        }
        [major, minor, patch] => {
            parse_u64(major)?;
            parse_u64(minor)?;
            parse_u64(patch)?;
            format!("{major}.{minor}.{patch}")
        }
        _ => return Err(format!("invalid partial version: {base}")),
    };
    let full = match pre {
        Some(pre) => format!("{version_str}{pre}"),
        None => version_str,
    };
    Version::parse(&full).map_err(|e| format!("invalid version '{full}': {e}"))
}

/// Count the number of meaningful (non-wildcard) version components,
/// stripping build metadata and prerelease before counting.
fn version_component_count(input: &str) -> usize {
    let clean = input.split('+').next().unwrap_or(input);
    let clean = if let Some(idx) = clean.find('-') {
        &clean[..idx]
    } else {
        clean
    };
    clean
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"))
        .count()
}

fn parse_u64(input: &str) -> Result<u64, String> {
    input
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("invalid number '{input}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_caret_range() {
        let r = ParsedRange::parse("^1.2.3").expect("parse");
        assert_eq!(r.parts.len(), 1);
        assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
    }

    #[test]
    fn parse_caret_zero_major() {
        let r = ParsedRange::parse("^0.1.2").expect("parse");
        assert_eq!(r.parts[0].max, Some(Version::new(0, 2, 0)));
    }

    #[test]
    fn parse_caret_zero_minor() {
        let r = ParsedRange::parse("^0.0.1").expect("parse");
        assert_eq!(r.parts[0].max, Some(Version::new(0, 0, 2)));
    }

    #[test]
    fn parse_tilde_range() {
        let r = ParsedRange::parse("~1.2.3").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
        assert_eq!(r.parts[0].max, Some(Version::new(1, 3, 0)));
    }

    #[test]
    fn parse_tilde_gt_alias() {
        let r = ParsedRange::parse("~>3.2.1").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(3, 2, 1));
        assert_eq!(r.parts[0].max, Some(Version::new(3, 3, 0)));
    }

    #[test]
    fn parse_tilde_major_only() {
        let r = ParsedRange::parse("~1").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
    }

    #[test]
    fn parse_or_range() {
        let r = ParsedRange::parse("^14.17.0 || ^16.10.0 || >=17.0.0").expect("parse");
        assert_eq!(r.parts.len(), 3);
        assert_eq!(r.parts[0].min, Version::new(14, 17, 0));
        assert_eq!(r.parts[1].min, Version::new(16, 10, 0));
        assert_eq!(r.parts[2].min, Version::new(17, 0, 0));
        assert!(r.parts[2].max.is_none());
    }

    #[test]
    fn parse_gte_partial() {
        let r = ParsedRange::parse(">=1.2").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 0));
        assert!(r.parts[0].max.is_none());
    }

    #[test]
    fn parse_lt_partial() {
        let r = ParsedRange::parse("<1").expect("parse");
        assert_eq!(r.parts[0].max, Some(Version::new(1, 0, 0)));
    }

    #[test]
    fn parse_eq_x_range() {
        let r = ParsedRange::parse("=0.7.x").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(0, 7, 0));
        assert_eq!(r.parts[0].max, Some(Version::new(0, 8, 0)));
    }

    #[test]
    fn parse_multi_comparator() {
        let r = ParsedRange::parse("~1.2.1 >=1.2.3").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
        assert_eq!(r.parts[0].max, Some(Version::new(1, 3, 0)));
    }

    #[test]
    fn satisfies_caret() {
        let r = ParsedRange::parse("^1.2.3").expect("parse");
        assert!(r.satisfies(&Version::new(1, 2, 3)));
        assert!(r.satisfies(&Version::new(1, 9, 0)));
        assert!(!r.satisfies(&Version::new(2, 0, 0)));
        assert!(!r.satisfies(&Version::new(1, 2, 2)));
    }

    #[test]
    fn satisfies_or_range() {
        let r = ParsedRange::parse("^14.17.0 || ^16.10.0 || >=17.0.0").expect("parse");
        assert!(r.satisfies(&Version::new(14, 17, 0)));
        assert!(r.satisfies(&Version::new(16, 14, 0)));
        assert!(r.satisfies(&Version::new(20, 0, 0)));
        assert!(!r.satisfies(&Version::new(15, 0, 0)));
    }
}
