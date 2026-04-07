//! Core engine constraint computation.

use riri_common::{EngineConstraintKey, Engines};
use riri_semver_range::{ParsedRange, VersionPrecision, restrictive_range};
use std::collections::HashMap;

/// Extract the constraint string for a given engine key from an `Engines` value.
///
/// Handles both object format (`{ "node": ">=14" }`) and array format (`["node >= 14"]`).
#[must_use]
pub fn get_constraint_from_engines(engines: &Engines, key: EngineConstraintKey) -> Option<String> {
    let key_str = key.to_string();
    match engines {
        Engines::Object(map) => map.get(&key_str).cloned(),
        Engines::Array(arr) => arr
            .iter()
            .find(|s| s.contains(&key_str))
            .map(|s| s.replace(&key_str, "").trim().to_string()),
    }
}

/// Compute the most restrictive engine range by folding over all entries.
///
/// Iterates `(package_name, engines)` pairs, extracts the constraint for the
/// given `key`, and progressively narrows the range using `restrictive_range`.
///
/// Returns the humanized range string, or `"*"` if no constraints were found.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn compute_engines_constraint<'a>(
    entries: impl Iterator<Item = (&'a str, &'a Engines)>,
    key: EngineConstraintKey,
    precision: VersionPrecision,
) -> String {
    let wildcard = ParsedRange::parse("*").expect("wildcard always parses");
    let mut most_restrictive = wildcard;
    let mut ignored_ranges: Vec<String> = Vec::new();

    for (_pkg_name, engines) in entries {
        let Some(constraint) = get_constraint_from_engines(engines, key) else {
            continue;
        };

        if constraint == "*" || constraint.is_empty() {
            continue;
        }

        // Skip ranges already proven to be supersets of the current most restrictive range.
        if ignored_ranges.contains(&constraint) {
            continue;
        }

        let Ok(range) = ParsedRange::parse(&constraint) else {
            continue;
        };

        let new_most_restrictive = restrictive_range(&most_restrictive, &range);
        if new_most_restrictive.humanize() == most_restrictive.humanize() {
            // Range didn't narrow most restrictive range — it's a superset or non-intersecting. Cache it.
            ignored_ranges.push(constraint);
        } else {
            most_restrictive = new_most_restrictive;
        }
    }

    most_restrictive.humanize_with(precision)
}

/// Input for [`check_engines`].
pub struct CheckEnginesInput<'a> {
    /// The lockfile engine entries: `(package_name, engines)` pairs.
    pub lockfile_entries: Vec<(&'a str, &'a Engines)>,
    /// The current `package.json` engines field (if any).
    pub package_engines: Option<&'a HashMap<String, String>>,
    /// Which engine keys to check. If empty, checks all (node, npm, yarn).
    pub filter_engines: Vec<EngineConstraintKey>,
    /// Version precision for humanized output.
    pub precision: VersionPrecision,
}

/// A single engine whose computed range differs from the current `package.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRangeToSet {
    pub engine: EngineConstraintKey,
    /// Current humanized range from `package.json` (or `"*"` if absent).
    pub range: String,
    /// New computed humanized range from all dependencies.
    pub range_to_set: String,
}

/// Output of [`check_engines`].
#[derive(Debug, Clone)]
pub struct CheckEnginesOutput {
    /// Engines that changed (current vs computed differ).
    pub engines_range_to_set: Vec<EngineRangeToSet>,
    /// The full computed engines map (engine key → humanized range).
    pub computed_engines: HashMap<EngineConstraintKey, String>,
}

/// Check engine constraints: compute the most restrictive ranges from lockfile
/// dependencies and compare against the current `package.json` engines.
///
/// For each engine key, the computation includes the root `package.json` engines
/// as the initial base, then folds over all lockfile entries.
#[must_use]
pub fn check_engines(input: &CheckEnginesInput<'_>) -> CheckEnginesOutput {
    let all_keys = [
        EngineConstraintKey::Node,
        EngineConstraintKey::Npm,
        EngineConstraintKey::Yarn,
    ];

    let keys: &[EngineConstraintKey] = if input.filter_engines.is_empty() {
        &all_keys
    } else {
        &input.filter_engines
    };

    let mut engines_range_to_set = Vec::new();
    let mut computed_engines = HashMap::new();

    for &key in keys {
        // Build the "from" range: just the root package.json engines
        let from = match input.package_engines {
            Some(pkg_engines) => {
                let root_engines = Engines::Object(pkg_engines.clone());
                let root_entries: Vec<(&str, &Engines)> = vec![("", &root_engines)];
                compute_engines_constraint(root_entries.into_iter(), key, input.precision)
            }
            None => "*".to_string(),
        };

        // Build the "to" range: root package.json engines + all lockfile entries
        let root_engines;
        let to = {
            root_engines = input
                .package_engines
                .cloned()
                .map_or_else(|| Engines::Object(HashMap::new()), Engines::Object);
            let combined = std::iter::once(("" as &str, &root_engines as &Engines))
                .chain(input.lockfile_entries.iter().copied());
            compute_engines_constraint(combined, key, input.precision)
        };

        computed_engines.insert(key, to.clone());

        if from != to {
            engines_range_to_set.push(EngineRangeToSet {
                engine: key,
                range: from,
                range_to_set: to,
            });
        }
    }

    CheckEnginesOutput {
        engines_range_to_set,
        computed_engines,
    }
}
