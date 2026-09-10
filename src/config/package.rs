//! Configuration package manifests, compatibility checks, and bounded file loading.

use super::ConfigError;
use super::generated::{SCHEMA_PARTS, TABLE_FILES};

mod files;
mod hashes;
mod manifest;

pub use files::{MAX_FILE_BYTES, MAX_MANIFEST_BYTES, MAX_PACKAGE_BYTES};
pub use hashes::package_schema_hash;
pub use manifest::{read_package, write_package_manifest};
