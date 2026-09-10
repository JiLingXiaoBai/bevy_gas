use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use super::TABLE_FILES;
use serde_json::{Map, Value, json};

use super::ConfigError;
use super::{
    files::{
        MAX_FILE_BYTES, MAX_MANIFEST_BYTES, MAX_PACKAGE_BYTES, add_package_size, read_limited,
    },
    hashes::{package_data_hash, package_schema_hash},
};

const FORMAT_VERSION: u64 = 1;
const TEMPLATE_VERSION: u64 = 1;
const MANIFEST_FILE: &str = "manifest.json";

pub(super) struct FileRecord {
    pub(super) name: String,
    pub(super) size: u64,
    pub(super) hash: String,
}

/// Writes `manifest.json` for the complete expected table set in `directory`.
///
/// Returns an error for missing or oversized files, invalid generated filenames,
/// allocation failures, or I/O errors. Call this on an unpublished export directory
/// and validate the package before publishing the directory.
pub fn write_package_manifest(directory: impl AsRef<Path>) -> Result<(), ConfigError> {
    let directory = directory.as_ref();
    let expected = expected_files()?;
    let mut total = 0;
    let mut files = Vec::new();
    for name in expected.keys() {
        let bytes = read_limited(
            &directory.join(name),
            MAX_FILE_BYTES.min(MAX_PACKAGE_BYTES - total),
        )?;
        let size = bytes.len() as u64;
        add_package_size(&mut total, size)?;
        files.push(FileRecord {
            name: name.clone(),
            size,
            hash: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    let data_hash = package_data_hash(&files);
    let file_values: Vec<Value> = files
        .into_iter()
        .map(|file| json!({"name": file.name, "size": file.size, "hash": file.hash}))
        .collect();
    let manifest = json!({
        "format_version": FORMAT_VERSION,
        "template_version": TEMPLATE_VERSION,
        "schema_hash": package_schema_hash(),
        "data_hash": data_hash,
        "files": file_values,
    });
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| ConfigError::new(MANIFEST_FILE, error.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "manifest exceeds its size limit",
        ));
    }
    fs::write(directory.join(MANIFEST_FILE), bytes)
        .map_err(|error| ConfigError::new(MANIFEST_FILE, error.to_string()))
}

/// Verifies and loads the package in `directory` into owned table byte buffers.
///
/// The returned map uses generated table names without the `.bytes` suffix.
/// Versions, schema, the complete expected table set, sizes, file hashes and the
/// combined data hash must match. Each table is read once; callers must decode the
/// returned buffers instead of rereading files after verification.
pub fn read_package(directory: impl AsRef<Path>) -> Result<BTreeMap<String, Vec<u8>>, ConfigError> {
    let directory = directory.as_ref();
    let manifest_bytes = read_limited(&directory.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| ConfigError::new(MANIFEST_FILE, error.to_string()))?;
    let root = object(&manifest, MANIFEST_FILE)?;
    require_fields(
        root,
        &[
            "format_version",
            "template_version",
            "schema_hash",
            "data_hash",
            "files",
        ],
        MANIFEST_FILE,
    )?;
    if integer(root, "format_version", MANIFEST_FILE)? != FORMAT_VERSION {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "unsupported format_version",
        ));
    }
    if integer(root, "template_version", MANIFEST_FILE)? != TEMPLATE_VERSION {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "unsupported template_version",
        ));
    }
    if string(root, "schema_hash", MANIFEST_FILE)? != package_schema_hash() {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "schema_hash does not match compiled schema",
        ));
    }
    let expected_data_hash = string(root, "data_hash", MANIFEST_FILE)?;
    let entries = root
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| ConfigError::new(MANIFEST_FILE, "files must be an array"))?;
    let expected = expected_files()?;
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    let mut declared_total = 0;
    for (index, entry) in entries.iter().enumerate() {
        let context = format!("{MANIFEST_FILE}.files[{index}]");
        let entry = object(entry, &context)?;
        require_fields(entry, &["name", "size", "hash"], &context)?;
        let name = string(entry, "name", &context)?;
        if !expected.contains_key(name) {
            return Err(ConfigError::new(
                &context,
                format!("unknown or unsafe table filename {name:?}"),
            ));
        }
        if !seen.insert(name) {
            return Err(ConfigError::new(
                &context,
                format!("duplicate table filename {name:?}"),
            ));
        }
        let size = integer(entry, "size", &context)?;
        if size > MAX_FILE_BYTES {
            return Err(ConfigError::new(
                &context,
                format!("table size exceeds limit {MAX_FILE_BYTES}"),
            ));
        }
        add_package_size(&mut declared_total, size)?;
        records.push(FileRecord {
            name: name.to_owned(),
            size,
            hash: string(entry, "hash", &context)?.to_owned(),
        });
    }
    if seen.len() != expected.len() {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "manifest does not list the complete expected table set",
        ));
    }
    records.sort_by(|left, right| left.name.cmp(&right.name));
    if package_data_hash(&records) != expected_data_hash {
        return Err(ConfigError::new(
            MANIFEST_FILE,
            "data_hash does not match the file manifest",
        ));
    }

    let mut loaded = BTreeMap::new();
    let mut total = 0;
    for record in records {
        let bytes = read_limited(&directory.join(&record.name), record.size)?;
        let size = bytes.len() as u64;
        add_package_size(&mut total, size)?;
        if size != record.size {
            return Err(ConfigError::new(
                &record.name,
                "file size does not match manifest",
            ));
        }
        if blake3::hash(&bytes).to_hex().as_str() != record.hash {
            return Err(ConfigError::new(
                &record.name,
                "file hash does not match manifest",
            ));
        }
        let table_name = expected.get(&record.name).ok_or_else(|| {
            ConfigError::new(&record.name, "table is absent from compiled schema")
        })?;
        loaded.insert((*table_name).to_owned(), bytes);
    }
    Ok(loaded)
}

fn expected_files() -> Result<BTreeMap<String, &'static str>, ConfigError> {
    let mut expected = BTreeMap::new();
    for &table in TABLE_FILES {
        if table.is_empty()
            || !table
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(ConfigError::new(
                "compiled schema",
                "table filename contains unsupported characters",
            ));
        }
        if expected.insert(format!("{table}.bytes"), table).is_some() {
            return Err(ConfigError::new(
                "compiled schema",
                "duplicate table filename",
            ));
        }
    }
    Ok(expected)
}

fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, ConfigError> {
    value
        .as_object()
        .ok_or_else(|| ConfigError::new(context, "expected a JSON object"))
}

fn require_fields(
    value: &Map<String, Value>,
    names: &[&str],
    context: &str,
) -> Result<(), ConfigError> {
    if value.len() != names.len() || names.iter().any(|name| !value.contains_key(*name)) {
        return Err(ConfigError::new(
            context,
            "object fields do not match the package format",
        ));
    }
    Ok(())
}

fn integer(value: &Map<String, Value>, name: &str, context: &str) -> Result<u64, ConfigError> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| ConfigError::new(context, format!("{name} must be an unsigned integer")))
}

fn string<'a>(
    value: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str, ConfigError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ConfigError::new(context, format!("{name} must be a string")))
}
