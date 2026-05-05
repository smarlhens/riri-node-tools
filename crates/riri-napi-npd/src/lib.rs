#![deny(clippy::all)]
// NAPI exports are consumed by the C ABI, not Rust — dead_code is a false positive for cdylib
#![allow(dead_code)]
// NAPI #[napi] functions require owned String parameters, not &str
#![allow(clippy::needless_pass_by_value)]
// Errors surface to JS via napi::Error::from_reason; documenting each is noise.
#![allow(clippy::missing_errors_doc)]

use napi_derive::napi;
use riri_common::{LockfileVersions, PackageJson};
use riri_npd::pin_dependencies as npd_pin_dependencies;

#[napi(object)]
pub struct DependencyPin {
    pub name: String,
    pub kind: String,
    pub from: String,
    pub to: String,
}

#[napi(object)]
pub struct PinDependenciesResult {
    pub pins: Vec<DependencyPin>,
}

#[napi(object)]
pub struct PinDependenciesOptions {
    pub package_json: String,
    pub lockfile_content: String,
    pub lockfile_type: Option<String>,
}

fn parse_lockfile_versions(
    content: &str,
    lockfile_type: Option<&str>,
) -> napi::Result<Box<dyn LockfileVersions>> {
    match lockfile_type.unwrap_or("npm") {
        "npm" => riri_npm::NpmPackageLock::parse(content)
            .map(|lock| Box::new(lock) as Box<dyn LockfileVersions>)
            .map_err(|error| {
                napi::Error::from_reason(format!("failed to parse npm lockfile: {error}"))
            }),
        "pnpm" => riri_pnpm::PnpmLockfile::parse(content)
            .map(|lock| Box::new(lock) as Box<dyn LockfileVersions>)
            .map_err(|error| {
                napi::Error::from_reason(format!("failed to parse pnpm lockfile: {error}"))
            }),
        "yarn" => Err(napi::Error::from_reason(
            "yarn lockfile parsing requires a directory path, not string content — use pinDependenciesFromPath instead",
        )),
        other => Err(napi::Error::from_reason(format!(
            "unknown lockfile type: {other}"
        ))),
    }
}

#[napi]
pub fn pin_dependencies(options: PinDependenciesOptions) -> napi::Result<PinDependenciesResult> {
    let package_json: PackageJson =
        serde_json::from_str(&options.package_json).map_err(|error| {
            napi::Error::from_reason(format!("failed to parse package.json: {error}"))
        })?;

    let lockfile =
        parse_lockfile_versions(&options.lockfile_content, options.lockfile_type.as_deref())?;

    let pins = npd_pin_dependencies(&package_json, lockfile.as_ref())
        .map_err(|error| napi::Error::from_reason(format!("pin_dependencies failed: {error}")))?
        .into_iter()
        .map(|pin| DependencyPin {
            name: pin.name,
            kind: pin.kind.as_str().to_string(),
            from: pin.current_range,
            to: pin.pinned_version,
        })
        .collect();

    Ok(PinDependenciesResult { pins })
}
