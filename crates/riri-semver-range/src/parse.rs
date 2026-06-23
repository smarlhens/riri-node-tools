use crate::{Op, RangePart};
use semver::Version;
use smallvec::{SmallVec, smallvec};

/// Storage for the parts of a [`ParsedRange`].
///
/// The vast majority of ranges are a single comparator set (no `||`), which is
/// kept inline on the stack; only disjunctions spill to the heap.
pub type Parts = SmallVec<[RangePart; 1]>;

/// A full semver range: multiple parts joined by `||`.
#[derive(Debug, Clone)]
pub struct ParsedRange {
    pub parts: Parts,
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
                parts: smallvec![wildcard()],
            });
        }

        let mut parts: Parts = input
            .split("||")
            .map(|s| parse_comparator_set(s.trim()))
            .collect::<Result<Parts, _>>()?;

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
            parts: self.parts[1..].iter().cloned().collect(),
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

    // Hyphen range: "1.0.0 - 2.0.0" (never operator-led).
    if !matches!(
        input.as_bytes().first(),
        Some(b'^' | b'~' | b'>' | b'<' | b'=')
    ) && let Some(idx) = input.find(" - ")
    {
        return parse_hyphen_range(&input[..idx], &input[idx + 3..]);
    }

    // Single fully-specified comparator: built directly; others fall through to tokenize.
    if let Some(part) = fast_comparator(input) {
        return Ok(part);
    }

    // Split into whitespace-separated tokens, merging bare operators with next token.
    // This handles ">= 1.0.0", "< 2.0.0", "~ 1.0", "^ 1.2", "~> 1", etc.
    let tokens = comparator_tokens(input);

    if tokens.len() == 1 {
        return parse_single_comparator(tokens[0]);
    }

    // Multi-comparator: intersect all (e.g., ">=1.0.0 <2.0.0" or "~1.2.1 >=1.2.3")
    // Parse each token as a comparator, then intersect by taking tightest bounds.
    let mut min = Version::new(0, 0, 0);
    let mut min_op = Op::Gte;
    let mut max: Option<Version> = None;
    let mut max_op: Option<Op> = None;

    for &token in &tokens {
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

/// Operator recognized by the byte-level fast path.
#[derive(Clone, Copy)]
enum FastOp {
    Caret,
    Tilde,
    Gte,
    Gt,
    Lt,
    Lte,
    Exact,
}

/// Fast path for an optional operator + fully-specified `major.minor.patch[-pre]`
/// version. Returns `None` for anything else (wildcards, partials, build metadata,
/// hyphen ranges) so the string parser handles it — a pure shortcut, no behaviour change.
fn fast_comparator(token: &str) -> Option<RangePart> {
    let bytes = token.as_bytes();
    let mut i = skip_ascii_ws(bytes, 0);

    let op = match (bytes.get(i).copied(), bytes.get(i + 1).copied()) {
        (Some(b'^'), _) => {
            i += 1;
            FastOp::Caret
        }
        (Some(b'~'), Some(b'>')) => {
            i += 2;
            FastOp::Tilde
        }
        (Some(b'~'), _) => {
            i += 1;
            FastOp::Tilde
        }
        (Some(b'>'), Some(b'=')) => {
            i += 2;
            FastOp::Gte
        }
        (Some(b'>'), _) => {
            i += 1;
            FastOp::Gt
        }
        (Some(b'<'), Some(b'=')) => {
            i += 2;
            FastOp::Lte
        }
        (Some(b'<'), _) => {
            i += 1;
            FastOp::Lt
        }
        (Some(b'='), _) => {
            i += 1;
            FastOp::Exact
        }
        (Some(b'0'..=b'9' | b'v' | b'V'), _) => FastOp::Exact,
        _ => return None,
    };

    i = skip_ascii_ws(bytes, i);
    if matches!(bytes.get(i), Some(b'v' | b'V')) {
        i += 1;
    }

    let (major, next) = take_u64(bytes, i)?;
    i = next;
    if bytes.get(i) != Some(&b'.') {
        return None;
    }
    i += 1;
    let (minor, next) = take_u64(bytes, i)?;
    i = next;
    if bytes.get(i) != Some(&b'.') {
        return None;
    }
    i += 1;
    let (patch, next) = take_u64(bytes, i)?;
    i = next;

    let version = match bytes.get(i).copied() {
        None => Version::new(major, minor, patch),
        Some(b'-') => {
            let pre = &token[i..];
            if pre.as_bytes().contains(&b'+') {
                return None;
            }
            Version::parse(&format!("{major}.{minor}.{patch}{pre}")).ok()?
        }
        _ => return None,
    };

    Some(build_full_version_part(op, version))
}

/// Upper bound (exclusive) for a caret over a fully-specified version.
fn caret_upper_bound(v: &Version) -> Version {
    if v.major == 0 {
        if v.minor == 0 {
            Version::new(0, 0, v.patch + 1)
        } else {
            Version::new(0, v.minor + 1, 0)
        }
    } else {
        Version::new(v.major + 1, 0, 0)
    }
}

/// Build the range part for an operator applied to a fully-specified version.
fn build_full_version_part(op: FastOp, v: Version) -> RangePart {
    let (min, min_op, max, max_op) = match op {
        FastOp::Caret => {
            let upper = caret_upper_bound(&v);
            (v, Op::Gte, Some(upper), Some(Op::Lt))
        }
        FastOp::Tilde => {
            let upper = Version::new(v.major, v.minor + 1, 0);
            (v, Op::Gte, Some(upper), Some(Op::Lt))
        }
        FastOp::Gte => (v, Op::Gte, None, None),
        FastOp::Gt => (v, Op::Gt, None, None),
        FastOp::Lt => (Version::new(0, 0, 0), Op::Gte, Some(v), Some(Op::Lt)),
        FastOp::Lte => (Version::new(0, 0, 0), Op::Gte, Some(v), Some(Op::Lte)),
        FastOp::Exact => return exact_version_part(v),
    };
    RangePart {
        min,
        min_op,
        max,
        max_op,
    }
}

fn skip_ascii_ws(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while matches!(
        bytes.get(i),
        Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
    ) {
        i += 1;
    }
    i
}

fn take_u64(bytes: &[u8], start: usize) -> Option<(u64, usize)> {
    let mut i = start;
    let mut value: u64 = 0;
    while let Some(&b @ b'0'..=b'9') = bytes.get(i) {
        value = value.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
        i += 1;
    }
    (i > start).then_some((value, i))
}

/// Parse a single comparator token.
fn parse_single_comparator(token: &str) -> Result<RangePart, String> {
    if let Some(part) = fast_comparator(token) {
        return Ok(part);
    }

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
    Ok(exact_version_part(v))
}

/// Build the range part for a fully-specified exact version (`1.2.3` or `=1.2.3`).
///
/// Release → `>=v <v.(patch+1)` (equivalent to `=v`). Prerelease → the point
/// interval `>=v <=v`, since node-semver matches only that exact prerelease.
fn exact_version_part(v: Version) -> RangePart {
    let (max, max_op) = if v.pre.is_empty() {
        (Version::new(v.major, v.minor, v.patch + 1), Op::Lt)
    } else {
        (v.clone(), Op::Lte)
    };
    RangePart {
        min: v,
        min_op: Op::Gte,
        max: Some(max),
        max_op: Some(max_op),
    }
}

/// Split a comparator set into borrowed tokens; a bare operator merges with the
/// following word as one slice (e.g. `">= 1.0.0"`), so no `String` is allocated.
fn comparator_tokens(input: &str) -> SmallVec<[&str; 2]> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut tokens: SmallVec<[&str; 2]> = SmallVec::new();
    let mut i = 0;

    while i < len {
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }
        let word_start = i;
        while i < len && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let word = &input[word_start..i];

        if is_bare_operator(word) {
            let mut j = i;
            while j < len && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < len {
                while j < len && !bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                tokens.push(&input[word_start..j]);
                i = j;
                continue;
            }
        }

        tokens.push(word);
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
    let mut dots = 0_usize;
    for b in token.bytes() {
        match b {
            b'x' | b'X' | b'*' => return true,
            b'.' => dots += 1,
            _ => {}
        }
    }
    dots < 2
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
    Ok(exact_version_part(v))
}

fn parse_caret(input: &str) -> Result<RangePart, String> {
    let (v, parts_count) = parse_partial_with_count(input)?;

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
        _ => caret_upper_bound(&v),
    };
    Ok(RangePart {
        min: v,
        min_op: Op::Gte,
        max: Some(upper),
        max_op: Some(Op::Lt),
    })
}

fn parse_tilde(input: &str) -> Result<RangePart, String> {
    let (v, parts_count) = parse_partial_with_count(input)?;
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
    parse_partial_with_count(input).map(|(version, _)| version)
}

/// Like [`parse_partial_version`], but also returns how many explicit numeric
/// components precede any wildcard (`0..=3`) — caret/tilde need it, from one scan.
fn parse_partial_with_count(input: &str) -> Result<(Version, usize), String> {
    let input = strip_v(input);
    let input = input.split('+').next().unwrap_or(input);

    let Some(idx) = input.find('-') else {
        let (major, minor, patch, count) = parse_core_components(input)?;
        return Ok((Version::new(major, minor, patch), count));
    };

    // Prerelease: reassemble core + suffix and parse via `semver` to keep identifiers.
    let (major, minor, patch, count) = parse_core_components(&input[..idx])?;
    let pre = &input[idx..];
    let full = format!("{major}.{minor}.{patch}{pre}");
    let version = Version::parse(&full).map_err(|e| format!("invalid version '{full}': {e}"))?;
    Ok((version, count))
}

/// Parse the numeric `major[.minor[.patch]]` core of a (possibly partial) version,
/// treating `x`/`X`/`*` and any absent components as `0`. Also returns how many
/// explicit numeric components were present before any wildcard.
fn parse_core_components(base: &str) -> Result<(u64, u64, u64, usize), String> {
    let mut components = base
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"));

    let Some(major) = components.next() else {
        return Ok((0, 0, 0, 0));
    };
    let major = parse_u64(major)?;

    let Some(minor) = components.next() else {
        return Ok((major, 0, 0, 1));
    };
    let minor = parse_u64(minor)?;

    let Some(patch) = components.next() else {
        return Ok((major, minor, 0, 2));
    };
    let patch = parse_u64(patch)?;

    if components.next().is_some() {
        return Err(format!("invalid partial version: {base}"));
    }
    Ok((major, minor, patch, 3))
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

    // --- Parsed-output assertions for prerelease ranges ---

    fn pre(s: &str) -> Version {
        Version::parse(s).expect("valid prerelease version")
    }

    #[test]
    fn parse_caret_prerelease() {
        let r = ParsedRange::parse("^1.2.3-alpha").expect("parse");
        assert_eq!(r.parts[0].min, pre("1.2.3-alpha"));
        assert_eq!(r.parts[0].min_op, Op::Gte);
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lt));
    }

    #[test]
    fn parse_tilde_prerelease() {
        let r = ParsedRange::parse("~1.2.3-beta").expect("parse");
        assert_eq!(r.parts[0].min, pre("1.2.3-beta"));
        assert_eq!(r.parts[0].max, Some(Version::new(1, 3, 0)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lt));
    }

    #[test]
    fn parse_gte_prerelease_is_open() {
        let r = ParsedRange::parse(">=1.2.3-rc.1").expect("parse");
        assert_eq!(r.parts[0].min, pre("1.2.3-rc.1"));
        assert_eq!(r.parts[0].min_op, Op::Gte);
        assert!(r.parts[0].max.is_none());
    }

    #[test]
    fn parse_hyphen_prerelease() {
        let r = ParsedRange::parse("1.2.3-alpha - 2.0.0").expect("parse");
        assert_eq!(r.parts[0].min, pre("1.2.3-alpha"));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lte));
    }

    #[test]
    fn parse_exact_prerelease_is_point_interval() {
        // node-semver matches only the exact prerelease version, so we emit `>=v <=v`.
        for input in ["1.2.3-alpha", "=1.2.3-alpha"] {
            let r = ParsedRange::parse(input).expect("parse");
            let v = pre("1.2.3-alpha");
            assert_eq!(r.parts[0].min, v, "{input}");
            assert_eq!(r.parts[0].min_op, Op::Gte, "{input}");
            assert_eq!(r.parts[0].max, Some(v), "{input}");
            assert_eq!(r.parts[0].max_op, Some(Op::Lte), "{input}");
        }
    }

    #[test]
    fn exact_prerelease_excludes_release_and_other_prereleases() {
        let r = ParsedRange::parse("1.2.3-alpha").expect("parse");
        assert!(r.satisfies(&pre("1.2.3-alpha")));
        assert!(!r.satisfies(&Version::new(1, 2, 3)));
        assert!(!r.satisfies(&pre("1.2.3-beta")));
        assert!(!r.satisfies(&pre("1.2.3-alpha.1")));
    }

    #[test]
    fn loose_prerelease_suffix_is_rejected() {
        // node-semver loose mode accepts "1.2.3beta" / "^1.2.3beta"; we deliberately
        // do not implement loose mode, so these must error rather than parse.
        assert!(ParsedRange::parse("1.2.3beta").is_err());
        assert!(ParsedRange::parse("^1.2.3beta").is_err());
        assert!(ParsedRange::parse("~1.2.3beta").is_err());
    }

    // --- Operator + x-range / whitespace / build-metadata edge cases ---

    #[test]
    fn parse_gt_x_range_bumps_to_next_major() {
        // >1.x means >=2.0.0
        let r = ParsedRange::parse(">1.x").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(2, 0, 0));
        assert_eq!(r.parts[0].min_op, Op::Gte);
        assert!(r.parts[0].max.is_none());
    }

    #[test]
    fn parse_lte_x_range() {
        // <=1.x means <2.0.0
        let r = ParsedRange::parse("<=1.x").expect("parse");
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lt));
    }

    #[test]
    fn parse_spaced_tilde_gt_alias() {
        let r = ParsedRange::parse("~> 1.2.3").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
        assert_eq!(r.parts[0].max, Some(Version::new(1, 3, 0)));
    }

    #[test]
    fn parse_spaced_caret_partial() {
        let r = ParsedRange::parse("^ 1.2").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 0));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
    }

    #[test]
    fn parse_build_metadata_stripped_on_exact() {
        let r = ParsedRange::parse("1.2.3+build.7").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 2, 3));
        assert_eq!(r.parts[0].max, Some(Version::new(1, 2, 4)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lt));
    }

    #[test]
    fn parse_hyphen_full_range() {
        let r = ParsedRange::parse("1.0.0 - 2.0.0").expect("parse");
        assert_eq!(r.parts[0].min, Version::new(1, 0, 0));
        assert_eq!(r.parts[0].max, Some(Version::new(2, 0, 0)));
        assert_eq!(r.parts[0].max_op, Some(Op::Lte));
    }
}
