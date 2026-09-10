//! Optional Luban configuration compiled into shared GAS runtime definitions.
//!
//! Enable the `luban-config` feature to load and inspect configuration packages.
//!
//! Load binary tables, validate their semantic constraints, and compile them once
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
pub use compiler::{ConfigError, compile_catalog, validate_tables};
pub use loading::{describe_ability, describe_ability_at_level, load_tables};
pub use package::{
    MAX_FILE_BYTES, MAX_MANIFEST_BYTES, MAX_PACKAGE_BYTES, package_schema_hash, read_package,
    write_package_manifest,
};
