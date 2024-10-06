use crate::types::{
    LockFileResult, NpmLock, PackageJson, PackageManager, PackageManagerLock, PnpmLock, YarnLockV2,
};
use detect_indent::{detect_indent, Indent};
use regex::Regex;
use serde_json::{Value as JsonValue, Value};
use serde_yml::Value as YamlValue;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use anyhow::{anyhow, Result};

pub fn parse_package(path: &PathBuf) -> Result<(PackageJson, Value, Indent)> {
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let indent = detect_indent(&contents);
    let package = serde_json::from_str(&contents)?;
    let raw = serde_json::from_str(&contents)?;

    Ok((package, raw, indent))
}

fn parse_npm_lock(path: &PathBuf) -> Result<NpmLock> {
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;

    let json: JsonValue = serde_json::from_str(&contents)?;

    match json.get("lockfileVersion") {
        Some(lockfile_version) => {
            let lockfile_version: u8 = serde_json::from_value(lockfile_version.clone())?;
            match lockfile_version {
                1 => Ok(NpmLock::Version1(serde_json::from_str(&contents)?)),
                2 => Ok(NpmLock::Version2(serde_json::from_str(&contents)?)),
                3 => Ok(NpmLock::Version3(serde_json::from_str(&contents)?)),
                _ => Err(anyhow!("Unsupported lockfile version")),
            }
        }
        None => Err(anyhow!("lockfileVersion field not found")),
    }
}

fn parse_yarn_lock(path: &PathBuf) -> Result<YarnLockV2> {
    let is_yarn_lock_v1 = Regex::new(r"# yarn lockfile v1")
        .expect("Failed to create regex pattern for identifying yarn lockfile v1");
    let is_yarn_lock_v2 = Regex::new(r"__metadata:\s*version: (\d)[\r\n]")
        .expect("Failed to create regex pattern for identifying yarn lockfile v2");

    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;

    if is_yarn_lock_v1.is_match(&contents) {
        Err(anyhow!("Yarn lock v1 parsing is not implemented yet."))
    } else if is_yarn_lock_v2.is_match(&contents) {
        Ok(serde_yml::from_str(&contents)?)
    } else {
        Err(anyhow!("Yarn lock file version parsing is not implemented yet."))
    }
}

fn deserialize_pnpm_lock_content_by_version(
    contents: &str,
    version: &str,
) -> Result<PnpmLock> {
    match version {
        "5.4" => Ok(PnpmLock::Version5(serde_yml::from_str(contents)?)),
        "6.0" => Ok(PnpmLock::Version6(serde_yml::from_str(contents)?)),
        _ => Err(anyhow!("Unsupported lockfile version")),
    }
}

fn parse_pnpm_lock(path: &PathBuf) -> Result<PnpmLock> {
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;

    let yaml: YamlValue = serde_yml::from_str(&contents)?;

    match yaml.get("lockfileVersion") {
        Some(lockfile_version) => match lockfile_version {
            YamlValue::Number(version_number) => {
                deserialize_pnpm_lock_content_by_version(&contents, &version_number.to_string())
            }
            YamlValue::String(version_str) => {
                deserialize_pnpm_lock_content_by_version(&contents, version_str)
            }
            _ => Err(anyhow!("Invalid lockfileVersion type")),
        },
        None => Err(anyhow!("lockfileVersion field not found")),
    }
}

pub fn parse_lock(lockfile_result: &LockFileResult) -> Result<PackageManagerLock> {
    match &lockfile_result.package_manager {
        PackageManager::Npm => parse_npm_lock(&lockfile_result.path).map(PackageManagerLock::Npm),
        PackageManager::Yarn => {
            parse_yarn_lock(&lockfile_result.path).map(PackageManagerLock::Yarn)
        }
        PackageManager::Pnpm => {
            parse_pnpm_lock(&lockfile_result.path).map(PackageManagerLock::Pnpm)
        }
    }
}
