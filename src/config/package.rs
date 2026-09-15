//! Bounded binary loading with optional package integrity validation.

#[cfg(feature = "config-validation")]
use super::generated::SCHEMA_PARTS;
use super::generated::TABLE_FILES;
use super::{ConfigError, ConfigErrorKind, ConfigLocation};

mod files;
#[cfg(feature = "config-validation")]
mod hashes;
#[cfg(feature = "config-validation")]
mod manifest;

#[cfg(feature = "config-validation")]
pub use files::MAX_MANIFEST_BYTES;
#[cfg(not(feature = "config-validation"))]
pub use files::read_package;
pub use files::{MAX_FILE_BYTES, MAX_PACKAGE_BYTES};
#[cfg(feature = "config-validation")]
pub use hashes::package_schema_hash;
#[cfg(feature = "config-validation")]
pub use manifest::{read_package, write_package_manifest};
