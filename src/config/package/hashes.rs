use super::SCHEMA_PARTS;
use blake3::Hasher;

use super::manifest::FileRecord;

/// Returns the BLAKE3 fingerprint of the compiled generated configuration schema.
///
/// Each generated schema part is prefixed with its byte length to preserve field
/// boundaries. Data packages must carry this exact hash to use these generated types.
pub fn package_schema_hash() -> String {
    let mut hasher = Hasher::new();
    update_part(&mut hasher, b"gas-config-schema-v1");
    for part in SCHEMA_PARTS {
        update_part(&mut hasher, part.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn package_data_hash(files: &[FileRecord]) -> String {
    let mut hasher = Hasher::new();
    update_part(&mut hasher, b"gas-config-data-v1");
    for file in files {
        update_part(&mut hasher, file.name.as_bytes());
        update_part(&mut hasher, &file.size.to_le_bytes());
        update_part(&mut hasher, file.hash.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn update_part(hasher: &mut Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
