use serde::Deserialize;
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

pub type Dependencies = HashMap<String, String>;

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PackageJson {
    pub name: String,
    pub dependencies: Option<Dependencies>,
    pub dev_dependencies: Option<Dependencies>,
    pub optional_dependencies: Option<Dependencies>,
}

#[derive(Debug, Deserialize, Clone)]
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
#[serde(untagged)]
pub enum VersionedDependencyOrResolved {
    Versioned(VersionedDependency),
    Resolved(ResolvedDependency),
}

pub type LockDependencies = HashMap<String, VersionedDependencyOrResolved>;
type NpmLockDependencies = LockDependencies;
type NpmLockPackages = LockDependencies;

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct NpmLockVersion1 {
    pub lockfile_version: u8,
    #[serde(default)]
    pub dependencies: NpmLockDependencies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct NpmLockVersion2 {
    pub lockfile_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<NpmLockPackages>,
    #[serde(default)]
    pub dependencies: NpmLockDependencies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct NpmLockVersion3 {
    pub lockfile_version: u8,
    #[serde(default)]
    pub packages: NpmLockPackages,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
#[serde(untagged)]
pub enum NpmLock {
    Version1(NpmLockVersion1),
    Version2(NpmLockVersion2),
    Version3(NpmLockVersion3),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct FirstLevelDependency {
    pub version: String,
    pub resolved: Option<String>,
    pub dependencies: Option<HashMap<String, String>>,
}

pub type YarnLockV2 = HashMap<String, FirstLevelDependency>;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmImporterV5 {
    pub dependencies: Option<HashMap<String, String>>,
    pub optional_dependencies: Option<HashMap<String, String>>,
    pub dev_dependencies: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmLockV5 {
    pub lockfile_version: String,
    pub importers: HashMap<String, PnpmImporterV5>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmImporterV6 {
    pub dependencies: Option<HashMap<String, VersionedDependency>>,
    pub optional_dependencies: Option<HashMap<String, VersionedDependency>>,
    pub dev_dependencies: Option<HashMap<String, VersionedDependency>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct PnpmLockV6 {
    pub lockfile_version: String,
    pub importers: HashMap<String, PnpmImporterV6>,
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
    Npm(NpmLock),
    Yarn(YarnLockV2),
    Pnpm(PnpmLock),
}
