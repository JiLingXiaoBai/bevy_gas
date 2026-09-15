use super::{
    ByteBuf, ConfigError, ConfigErrorKind, ConfigLocation, LubanError, Tables, read_package,
};
use std::path::Path;

/// Loads and decodes the expected binary tables from `directory`.
///
/// Without `config-validation`, reads `.bytes` files directly and ignores any manifest.
/// With the feature, requires a compatible manifest and verifies schema and hashes
/// before decoding. Each table buffer is read once and moved into its decoder.
/// Returns decoded rows, or a contextual file, decoding, or enabled validation error.
/// `compile_catalog` additionally validates authoring rules before registration
/// when the `config-validation` feature is enabled.
pub fn load_tables(directory: impl AsRef<Path>) -> Result<Tables, ConfigError> {
    let directory = directory.as_ref();
    let mut files = read_package(directory)?;
    let mut current_file = directory.to_owned();
    Tables::new(|name| {
        current_file = directory.join(format!("{name}.bytes"));
        let bytes = files
            .remove(name)
            .ok_or_else(|| LubanError::Loader(format!("missing table '{name}'")))?;
        Ok(ByteBuf::new(bytes))
    })
    .map_err(|error| {
        let location = ConfigLocation::file(current_file);
        let (kind, location) = match &error {
            LubanError::Decode(error) => {
                (ConfigErrorKind::Decode, location.at_byte(error.offset()))
            }
            LubanError::Loader(_) | LubanError::Table(_) => (ConfigErrorKind::Package, location),
        };
        ConfigError::new(kind, location, error.to_string())
    })
}
