use super::{ByteBuf, ConfigError, LubanError, Tables, read_package};
use std::path::Path;

/// Loads and decodes the expected binary tables from `directory`.
///
/// Without `luban-config`, reads `.bytes` files directly and ignores any manifest.
/// With the feature, requires a compatible manifest and verifies schema and hashes
/// before decoding. Each table buffer is read once and moved into its decoder.
/// Returns decoded rows, or a contextual file, decoding, or enabled validation error.
/// `compile_catalog` additionally validates authoring rules before registration
/// when the `luban-config` feature is enabled.
pub fn load_tables(directory: impl AsRef<Path>) -> Result<Tables, ConfigError> {
    let directory = directory.as_ref();
    let mut files = read_package(directory)?;
    Tables::new(|name| {
        let bytes = files
            .remove(name)
            .ok_or_else(|| LubanError::Loader(format!("missing table '{name}'")))?;
        Ok(ByteBuf::new(bytes))
    })
    .map_err(|error| ConfigError::new(directory.display().to_string(), error.to_string()))
}
