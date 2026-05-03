//! Data file schema versioning.

use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("data file schema version {found} does not match supported {SCHEMA_VERSION}")]
    UnsupportedVersion { found: u32 },
}
