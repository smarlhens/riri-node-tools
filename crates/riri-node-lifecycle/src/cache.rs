//! User-cache override loader.

use crate::data::{LifecycleData, LookupError};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache file unreadable: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache file invalid: {0}")]
    Parse(#[from] LookupError),
}

pub(crate) const CACHE_FILE_NAME: &str = "node-versions.json";

pub(crate) fn try_load(cache_dir: &Path) -> Option<LifecycleData> {
    let path = cache_dir.join(CACHE_FILE_NAME);
    let raw = std::fs::read_to_string(&path).ok()?;
    LifecycleData::parse(&raw).ok()
}
