//! Startup catalog construction on private registry snapshots.

use super::abilities::compile_abilities;
use super::effects::compile_effects;
use super::registration::{register_additional_cost_resources, register_attributes, register_tags};
use super::targeting::compile_targeting;
#[cfg(feature = "config-validation")]
use super::validate_prepared;
use super::{
    ConfigError, ConfigErrorKind, ConfigLocation, GameplayCatalog, PreparedTables, Tables,
};
use crate::{AttributeIdManager, GameplayTagManager, UniqueNamePool};
use bevy::prelude::World;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Prepares generated rows and compiles shared runtime definitions.
///
/// The world must contain UniqueNamePool, GameplayTagManager, and AttributeIdManager,
/// normally installed by GameplayAbilitySystemPlugin. Returns an unpublished catalog
/// or a classified error. Every returned error leaves all World resources unchanged.
///
/// Registration and construction run on private registry snapshots. Only a complete
/// successful catalog commits those snapshots; no resource is removed while preparing.
/// With config-validation, authoring rules are checked before registry construction.
/// Call during startup; replacing catalogs during combat remains unsupported.
pub fn compile_catalog(tables: &Tables, world: &mut World) -> Result<GameplayCatalog, ConfigError> {
    let prepared = PreparedTables::new(tables)?;
    #[cfg(feature = "config-validation")]
    validate_prepared(&prepared)?;

    let mut names = world
        .get_resource::<UniqueNamePool>()
        .cloned()
        .ok_or_else(|| missing_registry("UniqueNamePool"))?;
    let mut tag_manager = world
        .get_resource::<GameplayTagManager>()
        .cloned()
        .ok_or_else(|| missing_registry("GameplayTagManager"))?;
    let mut attribute_manager = world
        .get_resource::<AttributeIdManager>()
        .cloned()
        .ok_or_else(|| missing_registry("AttributeIdManager"))?;

    // The snapshots serve as both registration preflight and the eventual commit.
    let tags = register_tags(tables, &mut names, &mut tag_manager)?;
    let attributes = register_attributes(tables, &mut names, &mut attribute_manager)?;
    let additional_cost_resources = register_additional_cost_resources(tables, &mut names)?;
    let effects = compile_effects(&prepared, &tags, &attributes)?;
    let mut targeting = BTreeMap::new();
    for row in tables.tb_targeting.iter() {
        targeting.insert(row.id, Arc::new(compile_targeting(row, &tags)?));
    }
    let abilities = compile_abilities(
        &prepared,
        &tags,
        &effects,
        &targeting,
        &additional_cost_resources,
    )?;
    let catalog = GameplayCatalog {
        abilities,
        effects,
        attributes,
        tags,
    };

    world.insert_resource(names);
    world.insert_resource(tag_manager);
    world.insert_resource(attribute_manager);
    Ok(catalog)
}

fn missing_registry(name: &'static str) -> ConfigError {
    ConfigError::new(
        ConfigErrorKind::MissingResource,
        ConfigLocation::Resource(name),
        "install GameplayAbilitySystemPlugin before compiling configuration",
    )
}
