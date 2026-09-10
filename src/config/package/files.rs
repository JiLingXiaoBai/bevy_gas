use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use super::ConfigError;

/// Maximum byte size of one binary table file (64 MiB).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum combined byte size of all binary tables in one package (256 MiB).
pub const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum byte size of the JSON package manifest (1 MiB).
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

pub(super) fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ConfigError> {
    let context = path.display().to_string();
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ConfigError::new(&context, error.to_string()))?;
    if !metadata.is_file() {
        return Err(ConfigError::new(&context, "expected a regular file"));
    }
    if metadata.len() > limit {
        return Err(ConfigError::new(
            &context,
            format!("file size {} exceeds limit {limit}", metadata.len()),
        ));
    }
    let mut file =
        File::open(path).map_err(|error| ConfigError::new(&context, error.to_string()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| ConfigError::new(&context, error.to_string()))?;
    if !opened_metadata.is_file() || opened_metadata.len() > limit {
        return Err(ConfigError::new(
            &context,
            "opened file exceeds its size or file-type limit",
        ));
    }

    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        if count == 0 {
            break;
        }
        let next_length = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or_else(|| ConfigError::new(&context, "file length overflow"))?;
        if next_length > limit {
            return Err(ConfigError::new(
                &context,
                format!("file grew beyond limit {limit}"),
            ));
        }
        bytes.try_reserve(count).map_err(|error| {
            ConfigError::new(&context, format!("file allocation failed: {error}"))
        })?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.len() as u64 != opened_metadata.len() {
        return Err(ConfigError::new(
            &context,
            "file size changed during reading",
        ));
    }
    Ok(bytes)
}

pub(super) fn add_package_size(total: &mut u64, size: u64) -> Result<(), ConfigError> {
    *total = total
        .checked_add(size)
        .ok_or_else(|| ConfigError::new("configuration package", "combined file size overflow"))?;
    if *total > MAX_PACKAGE_BYTES {
        return Err(ConfigError::new(
            "configuration package",
            format!("combined file size exceeds limit {MAX_PACKAGE_BYTES}"),
        ));
    }
    Ok(())
}
