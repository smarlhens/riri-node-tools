use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

#[derive(Debug)]
pub struct LockFileResult {
    pub path: PathBuf,
    pub package_manager: PackageManager,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct Dependencies {
    #[serde(flatten)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PackageJson {
    #[serde(flatten)]
    pub dependencies: Option<Dependencies>,
    #[serde(flatten)]
    pub dev_dependencies: Option<Dependencies>,
    #[serde(flatten)]
    pub optional_dependencies: Option<Dependencies>,
}

#[derive(Debug, Deserialize)]
pub struct VersionedDependency {
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ResolvedDependency {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    pub link: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
#[serde(untagged)]
pub enum LockDependency {
    Versioned(VersionedDependency),
    Resolved(ResolvedDependency),
}
type PackageLockDependencies = HashMap<String, VersionedDependency>;
type PackageLockPackages = HashMap<String, VersionedDependencyOrResolved>;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum VersionedDependencyOrResolved {
    Versioned(VersionedDependency),
    Resolved(ResolvedDependency),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PackageLockVersion1 {
    pub lockfile_version: u8,
    #[serde(default)]
    pub dependencies: PackageLockDependencies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PackageLockVersion2 {
    pub lockfile_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<PackageLockPackages>,
    #[serde(default)]
    pub dependencies: PackageLockDependencies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PackageLockVersion3 {
    pub lockfile_version: u8,
    #[serde(default)]
    pub packages: PackageLockPackages,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
#[serde(untagged)]
pub enum PackageLock {
    Version1(PackageLockVersion1),
    Version2(PackageLockVersion2),
    Version3(PackageLockVersion3),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]

pub struct YarnLock {
    #[serde(rename = "type")]
    pub lock_type: String,
    pub object: LockFileObject,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFileObject {
    // Define the fields of LockFileObject as needed
}

#[derive(Debug, Deserialize)]
pub struct PnpmLockV5 {
    pub lockfile_version: String,
    pub importers: HashMap<String, PnpmImporterV5>,
}

#[derive(Debug, Deserialize)]
pub struct PnpmLockV6 {
    pub lockfile_version: String,
    pub importers: HashMap<String, PnpmImporterV6>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmImporterV5 {
    #[serde(flatten)]
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub optional_dependencies: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub dev_dependencies: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmImporterV6 {
    #[serde(flatten)]
    pub dependencies: Option<HashMap<String, VersionedDependency>>,
    #[serde(flatten)]
    pub optional_dependencies: Option<HashMap<String, VersionedDependency>>,
    #[serde(flatten)]
    pub dev_dependencies: Option<HashMap<String, VersionedDependency>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PnpmLock {
    Version5(PnpmLockV5),
    Version6(PnpmLockV6),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PackageManagerLock {
    Npm(PackageLock),
    Yarn(YarnLock),
    Pnpm(PnpmLock),
}
