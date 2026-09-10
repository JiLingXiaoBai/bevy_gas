use super::{ByteBuf, ConfigError, LubanError, Tables, read_package};
use std::path::Path;

/// Verifies and decodes a complete generated package from `directory`.
///
/// Requires a compatible manifest and verifies the complete table set and byte
/// hashes before decoding. Each table buffer is read once and moved into its decoder.
/// Returns decoded rows, or a contextual manifest, file, or decoder error.
/// `compile_catalog` additionally validates gameplay semantics before registration.
pub fn load_tables(directory: impl AsRef<Path>) -> Result<Tables, ConfigError> {
    let directory = directory.as_ref();
    let mut files = read_package(directory)?;
    Tables::new(|name| {
        let bytes = files
            .remove(name)
            .ok_or_else(|| LubanError::Loader(format!("missing verified table '{name}'")))?;
        Ok(ByteBuf::new(bytes))
    })
    .map_err(|error| ConfigError::new(directory.display().to_string(), error.to_string()))
}
