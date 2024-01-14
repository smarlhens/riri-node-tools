use core::{
    LockFileResult, NpmLock, PackageJson, PackageManager, PackageManagerLock, PnpmLock, YarnLockV2,
};
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub fn parse_package(path: &PathBuf) -> Result<PackageJson, Box<dyn Error>> {
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let package: PackageJson = serde_json::from_str(&contents)?;

    Ok(package)
}

fn parse_npm_lock(path: &PathBuf) -> Result<NpmLock, Box<dyn Error>> {
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
                _ => Err("Unsupported lockfile version".into()),
            }
        }
        None => Err("lockfileVersion field not found".into()),
    }
}

fn parse_yarn_lock(path: &PathBuf) -> Result<YarnLockV2, Box<dyn Error>> {
    let is_yarn_lock_v1 = Regex::new(r"# yarn lockfile v1").unwrap();
    let is_yarn_lock_v2 = Regex::new(r"__metadata:\s*version: (\d)[\r\n]").unwrap();

    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;

    match () {
        () if is_yarn_lock_v1.is_match(&contents) => {
            Err("Yarn lock v1 parsing is not implemented yet.".into())
        }
        () if is_yarn_lock_v2.is_match(&contents) => Ok(serde_yaml::from_str(&contents)?),
        () => Err("Yarn lock file version parsing is not implemented yet.".into()),
    }
}

fn deserialize_pnpm_lock_content_by_version(
    contents: &str,
    version: &str,
) -> Result<PnpmLock, Box<dyn Error>> {
    match version {
        "5.4" => Ok(PnpmLock::Version5(serde_yaml::from_str(contents)?)),
        "6.0" => Ok(PnpmLock::Version6(serde_yaml::from_str(contents)?)),
        _ => Err("Unsupported lockfile version".into()),
    }
}

fn parse_pnpm_lock(path: &PathBuf) -> Result<PnpmLock, Box<dyn Error>> {
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;

    let yaml: YamlValue = serde_yaml::from_str(&contents)?;

    match yaml.get("lockfileVersion") {
        Some(lockfile_version) => match lockfile_version {
            YamlValue::Number(version_number) => {
                deserialize_pnpm_lock_content_by_version(&contents, &version_number.to_string())
            }
            YamlValue::String(version_str) => {
                deserialize_pnpm_lock_content_by_version(&contents, version_str)
            }
            _ => Err("Invalid lockfileVersion type".into()),
        },
        None => Err("lockfileVersion field not found".into()),
    }
}

pub fn parse_lock(lockfile_result: &LockFileResult) -> Result<PackageManagerLock, Box<dyn Error>> {
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
