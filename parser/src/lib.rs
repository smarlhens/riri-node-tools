use core::{
    LockFileResult, Package, PackageLock, PackageLockVersion1, PackageLockVersion2,
    PackageLockVersion3, PackageManager, PackageManagerLock, PnpmLock, YarnLock,
};
use serde_json::{self, Value};
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub fn parse_package(path: &PathBuf) -> Result<Package, Box<dyn Error>> {
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let package: Package = serde_json::from_str(&contents)?;

    Ok(package)
}

fn parse_npm_lock(path: &PathBuf) -> Result<PackageLock, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let json: Value = serde_json::from_str(&contents)?;

    match json.get("lockfileVersion") {
        Some(lockfile_version) => {
            let lockfile_version: i32 = serde_json::from_value(lockfile_version.clone())?;
            match lockfile_version {
                1 => {
                    let package_lock: PackageLockVersion1 = serde_json::from_str(&contents)?;
                    Ok(PackageLock::Version1(package_lock))
                }
                2 => {
                    let package_lock: PackageLockVersion2 = serde_json::from_str(&contents)?;
                    Ok(PackageLock::Version2(package_lock))
                }
                3 => {
                    let package_lock: PackageLockVersion3 = serde_json::from_str(&contents)?;
                    Ok(PackageLock::Version3(package_lock))
                }
                _ => Err("Unsupported lockfile version".into()),
            }
        }
        None => Err("lockfileVersion field not found".into()),
    }
}

fn parse_yarn_lock(_path: &PathBuf) -> Result<YarnLock, Box<dyn Error>> {
    Err("Yarn lock parsing not implemented yet.".into())
}

fn parse_pnpm_lock(_path: &PathBuf) -> Result<PnpmLock, Box<dyn Error>> {
    Err("Pnpm lock parsing not implemented yet.".into())
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
