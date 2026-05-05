use napi_derive::napi;
use riri_common::{EngineConstraintKey, Engines, LockfileEngines, PackageJson};
use riri_nce::{CheckEnginesInput, check_engines as nce_check_engines};
use riri_semver_range::VersionPrecision;
use std::collections::HashMap;

#[napi(object)]
pub struct EngineChange {
    pub engine: String,
    pub from: String,
    pub to: String,
}

#[napi(object)]
pub struct CheckEnginesResult {
    pub computed_engines: HashMap<String, String>,
    pub changes: Vec<EngineChange>,
}

#[napi(object)]
pub struct CheckEnginesOptions {
    pub package_json: String,
    pub lockfile_content: String,
    pub lockfile_type: Option<String>,
    pub filter_engines: Option<Vec<String>>,
    pub precision: Option<String>,
}

fn parse_precision(input: Option<&str>) -> VersionPrecision {
    match input {
        Some("major") => VersionPrecision::Major,
        Some("minor") => VersionPrecision::MajorMinor,
        _ => VersionPrecision::Full,
    }
}

fn parse_lockfile_engines(
    content: &str,
    lockfile_type: Option<&str>,
) -> napi::Result<Box<dyn LockfileEngines>> {
    match lockfile_type.unwrap_or("npm") {
        "npm" => riri_npm::NpmPackageLock::parse(content)
            .map(|lock| Box::new(lock) as Box<dyn LockfileEngines>)
            .map_err(|error| {
                napi::Error::from_reason(format!("failed to parse npm lockfile: {error}"))
            }),
        "pnpm" => riri_pnpm::PnpmLockfile::parse(content)
            .map(|lock| Box::new(lock) as Box<dyn LockfileEngines>)
            .map_err(|error| {
                napi::Error::from_reason(format!("failed to parse pnpm lockfile: {error}"))
            }),
        "yarn" => Err(napi::Error::from_reason(
            "yarn lockfile parsing requires a directory path, not string content — use the CLI instead",
        )),
        other => Err(napi::Error::from_reason(format!(
            "unknown lockfile type: {other}"
        ))),
    }
}

#[napi]
pub fn check_engines(options: CheckEnginesOptions) -> napi::Result<CheckEnginesResult> {
    let package_json: PackageJson =
        serde_json::from_str(&options.package_json).map_err(|error| {
            napi::Error::from_reason(format!("failed to parse package.json: {error}"))
        })?;

    let lockfile =
        parse_lockfile_engines(&options.lockfile_content, options.lockfile_type.as_deref())?;

    let entries: Vec<(&str, &Engines)> = lockfile.engines_iter().collect();

    let filter_engines: Vec<EngineConstraintKey> = options
        .filter_engines
        .unwrap_or_default()
        .iter()
        .filter_map(|key| EngineConstraintKey::from_str_lowercase(key))
        .collect();

    let precision = parse_precision(options.precision.as_deref());

    let input = CheckEnginesInput {
        lockfile_entries: entries,
        package_engines: package_json.engines.as_ref(),
        filter_engines,
        precision,
    };

    let output = nce_check_engines(&input);

    let computed_engines = output
        .computed_engines
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();

    let changes = output
        .engines_range_to_set
        .into_iter()
        .map(|change| EngineChange {
            engine: change.engine.to_string(),
            from: change.range,
            to: change.range_to_set,
        })
        .collect();

    Ok(CheckEnginesResult {
        computed_engines,
        changes,
    })
}
