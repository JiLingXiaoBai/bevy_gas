//! Bounded binary loading with optional package integrity validation.

use super::ConfigError;
#[cfg(feature = "luban-config")]
use super::generated::SCHEMA_PARTS;
use super::generated::TABLE_FILES;

mod files;
#[cfg(feature = "luban-config")]
mod hashes;
#[cfg(feature = "luban-config")]
mod manifest;

#[cfg(feature = "luban-config")]
pub use files::MAX_MANIFEST_BYTES;
#[cfg(not(feature = "luban-config"))]
pub use files::read_package;
pub use files::{MAX_FILE_BYTES, MAX_PACKAGE_BYTES};
#[cfg(feature = "luban-config")]
pub use hashes::package_schema_hash;
#[cfg(feature = "luban-config")]
pub use manifest::{read_package, write_package_manifest};
