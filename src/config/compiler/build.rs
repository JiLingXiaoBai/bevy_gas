//! Catalog compilation order and ECS registry access.

use super::abilities::compile_abilities;
use super::effects::compile_effects;
use super::registration::{register_attributes, register_tags};
use super::targeting::compile_targeting;
#[cfg(feature = "luban-config")]
use super::validate_tables;
use super::{ConfigError, GameplayCatalog, Tables};
use crate::{AttributeIdManager, GameplayTagManager, UniqueNamePool};
use bevy::prelude::World;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Registers stable names from `tables` and compiles shared runtime definitions.
///
/// `world` must contain `UniqueNamePool`, `GameplayTagManager`, and
/// `AttributeIdManager`, normally installed by `GameplayAbilitySystemPlugin`.
/// Returns an unpublished catalog, or a contextual configuration/registration error.
/// With `luban-config`, full authoring validation happens before mutation.
/// Without it, only required runtime construction checks are performed.
/// Registration is append-only; any later construction or registration failure
/// may leave successfully registered names, but never publishes a partial catalog.
/// Call this during startup; replacing catalogs during combat is unsupported.
pub fn compile_catalog(tables: &Tables, world: &mut World) -> Result<GameplayCatalog, ConfigError> {
    #[cfg(feature = "luban-config")]
    validate_tables(tables)?;
    if !world.contains_resource::<GameplayTagManager>()
        || !world.contains_resource::<AttributeIdManager>()
    {
        return Err(ConfigError::new(
            "runtime registries",
            "install GameplayAbilitySystemPlugin before compiling configuration",
        ));
    }
    let mut names = world.remove_resource::<UniqueNamePool>().ok_or_else(|| {
        ConfigError::new(
            "UniqueNamePool",
            "install GameplayAbilitySystemPlugin before compiling configuration",
        )
    })?;
    let result = compile_with_names(tables, world, &mut names);
    world.insert_resource(names);
    result
}

fn compile_with_names(
    tables: &Tables,
    world: &mut World,
    names: &mut UniqueNamePool,
) -> Result<GameplayCatalog, ConfigError> {
    let tags = {
        let mut manager = world
            .get_resource_mut::<GameplayTagManager>()
            .ok_or_else(|| {
                ConfigError::new("GameplayTagManager", "required registry is missing")
            })?;
        register_tags(tables, names, &mut manager)?
    };
    let attributes = {
        let mut manager = world
            .get_resource_mut::<AttributeIdManager>()
            .ok_or_else(|| {
                ConfigError::new("AttributeIdManager", "required registry is missing")
            })?;
        register_attributes(tables, names, &mut manager)?
    };
    let effects = compile_effects(tables, &tags, &attributes)?;
    let mut targeting = BTreeMap::new();
    for row in tables.tb_targeting.iter() {
        targeting.insert(row.id, Arc::new(compile_targeting(row, &tags)?));
    }
    let abilities = compile_abilities(tables, &tags, &effects, &targeting)?;
    Ok(GameplayCatalog {
        abilities,
        effects,
        attributes,
        tags,
    })
}
