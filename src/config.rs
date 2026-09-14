//! Luban configuration compiled into shared GAS runtime definitions.
//!
//! Runtime loading, decoding, compilation, and grants are always available.
//! Enable `luban-config` for package integrity and authoring validation, manifest
//! creation, and human-readable inspection. Without it, loading reads binary
//! tables directly without reading a manifest or using BLAKE3 or JSON parsing.
//! Decoding bounds and runtime construction errors are checked in both modes.
//!
//! Load binary tables and compile them once
//! during startup before granting abilities to actors. Registry insertion is
//! append-only; failed registration may leave names registered, but never publishes
//! a partially compiled catalog. Runtime catalog replacement is not supported.

mod catalog;
mod compiler;
mod decoding;
mod loading;
mod package;

/// Generated configuration rows and table indexes for the compiled schema.
#[path = "../config/generated/mod.rs"]
pub mod generated;

pub use catalog::{
    AbilityId, CompiledAbility, ConfiguredAbilities, EffectId, GameplayCatalog, grant_ability,
    revoke_ability,
};
pub use compiler::{ConfigError, compile_catalog};
pub use loading::load_tables;
pub use package::{MAX_FILE_BYTES, MAX_PACKAGE_BYTES, read_package};

#[cfg(feature = "luban-config")]
pub use compiler::validate_tables;
#[cfg(feature = "luban-config")]
pub use loading::{describe_ability, describe_ability_at_level};
#[cfg(feature = "luban-config")]
pub use package::{MAX_MANIFEST_BYTES, package_schema_hash, write_package_manifest};
