use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

#[cfg(not(feature = "config-validation"))]
use super::TABLE_FILES;
use super::{ConfigError, ConfigErrorKind, ConfigLocation};
#[cfg(not(feature = "config-validation"))]
use std::collections::BTreeMap;

/// Maximum byte size of one binary table file (64 MiB).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum combined byte size of all binary tables in one package (256 MiB).
pub const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum byte size of the JSON package manifest (1 MiB).
#[cfg(feature = "config-validation")]
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Reads the expected binary tables from `directory` without consulting a manifest.
///
/// The returned map uses generated table names without the `.bytes` suffix.
/// Returns an error for missing files, I/O failures, or file/package size limits.
/// This implementation is used without `config-validation`; the feature-enabled
/// implementation additionally verifies the manifest, schema, and content hashes.
#[cfg(not(feature = "config-validation"))]
pub fn read_package(directory: impl AsRef<Path>) -> Result<BTreeMap<String, Vec<u8>>, ConfigError> {
    let directory = directory.as_ref();
    let mut loaded = BTreeMap::new();
    let mut total = 0;
    for &name in TABLE_FILES {
        let bytes = read_limited(
            &directory.join(format!("{name}.bytes")),
            MAX_FILE_BYTES.min(MAX_PACKAGE_BYTES - total),
        )?;
        add_package_size(&mut total, bytes.len() as u64)?;
        loaded.insert(name.to_owned(), bytes);
    }
    Ok(loaded)
}

pub(super) fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ConfigError> {
    let context = ConfigLocation::file(path);
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ConfigError::new(ConfigErrorKind::Io, &context, error.to_string()))?;
    if !metadata.is_file() {
        return Err(ConfigError::new(
            ConfigErrorKind::Package,
            &context,
            "expected a regular file",
        ));
    }
    if metadata.len() > limit {
        return Err(ConfigError::new(
            ConfigErrorKind::Capacity,
            &context,
            format!("file size {} exceeds limit {limit}", metadata.len()),
        ));
    }
    let mut file = File::open(path)
        .map_err(|error| ConfigError::new(ConfigErrorKind::Io, &context, error.to_string()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| ConfigError::new(ConfigErrorKind::Io, &context, error.to_string()))?;
    if !opened_metadata.is_file() || opened_metadata.len() > limit {
        return Err(ConfigError::new(
            ConfigErrorKind::Capacity,
            &context,
            "opened file exceeds its size or file-type limit",
        ));
    }

    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| ConfigError::new(ConfigErrorKind::Io, &context, error.to_string()))?;
        if count == 0 {
            break;
        }
        let next_length = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or_else(|| {
                ConfigError::new(ConfigErrorKind::Capacity, &context, "file length overflow")
            })?;
        if next_length > limit {
            return Err(ConfigError::new(
                ConfigErrorKind::Capacity,
                &context,
                format!("file grew beyond limit {limit}"),
            ));
        }
        bytes.try_reserve(count).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::Capacity,
                &context,
                format!("file allocation failed: {error}"),
            )
        })?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.len() as u64 != opened_metadata.len() {
        return Err(ConfigError::new(
            ConfigErrorKind::Capacity,
            &context,
            "file size changed during reading",
        ));
    }
    Ok(bytes)
}

pub(super) fn add_package_size(total: &mut u64, size: u64) -> Result<(), ConfigError> {
    *total = total.checked_add(size).ok_or_else(|| {
        ConfigError::new(
            ConfigErrorKind::Capacity,
            ConfigLocation::Operation("configuration package"),
            "combined file size overflow",
        )
    })?;
    if *total > MAX_PACKAGE_BYTES {
        return Err(ConfigError::new(
            ConfigErrorKind::Capacity,
            ConfigLocation::Operation("configuration package"),
            format!("combined file size exceeds limit {MAX_PACKAGE_BYTES}"),
        ));
    }
    Ok(())
}
