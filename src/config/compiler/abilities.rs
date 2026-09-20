//! Ability definitions, external-resource requirements, and ordered startup timelines.

use super::effects::resolve_effect;
use super::registration::resolve_tags;
use super::{
    AbilityId, CompiledAbility, ConfigError, ConfigErrorKind, ConfigLocation, EffectId,
    PreparedActionKind, PreparedTables, PreparedTargetScope,
};
use crate::{
    AbilityTags, AbilityTaskDef, AbilityTaskOnFinishedDef, AdditionalCost, GameplayAbility,
    GameplayEffect, GameplayTag, TargetingDefinition, UniqueName,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn compile_abilities(
    prepared: &PreparedTables,
    tags: &BTreeMap<String, GameplayTag>,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
    targeting: &BTreeMap<i32, Arc<TargetingDefinition>>,
    additional_cost_resources: &BTreeMap<&str, UniqueName>,
) -> Result<BTreeMap<AbilityId, CompiledAbility>, ConfigError> {
    let mut abilities = BTreeMap::new();
    for row in prepared.tables.tb_ability.iter() {
        let ability_tags = AbilityTags::default()
            .with_ability_asset_tags(resolve_tags(&row.asset_tags, tags)?)
            .with_activation_required_tags(resolve_tags(&row.required_tags, tags)?)
            .with_activation_blocked_tags(resolve_tags(&row.blocked_tags, tags)?)
            .with_cancel_abilities_with_tags(resolve_tags(&row.cancel_ability_tags, tags)?)
            .with_block_abilities_with_tags(resolve_tags(&row.block_ability_tags, tags)?);
        let mut definition = GameplayAbility::default()
            .with_tags(ability_tags)
            .with_startup_tasks(compile_tasks(prepared, row.id, effects)?)
            .with_additional_costs(compile_additional_costs(
                prepared,
                row.id,
                additional_cost_resources,
            )?)
            .with_allow_multiple_instances(row.allow_multiple_instances);
        if let Some(id) = row.cost_effect_id {
            definition = definition.with_cost(resolve_effect(id, effects)?);
        }
        if let Some(id) = row.cooldown_effect_id {
            definition = definition.with_cooldown(resolve_effect(id, effects)?);
        }
        let target = targeting.get(&row.targeting_id).ok_or_else(|| {
            ConfigError::new(
                ConfigErrorKind::Reference,
                ConfigLocation::table("Ability")
                    .row(row.id)
                    .field("targeting_id"),
                "unresolved targeting",
            )
        })?;
        let max_level = u32::try_from(row.max_level).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::InvalidValue,
                ConfigLocation::table("Ability")
                    .row(row.id)
                    .field("max_level"),
                error.to_string(),
            )
        })?;
        if max_level == 0 {
            return Err(ConfigError::new(
                ConfigErrorKind::InvalidValue,
                ConfigLocation::table("Ability")
                    .row(row.id)
                    .field("max_level"),
                "maximum level must be positive",
            ));
        }
        abilities.insert(
            AbilityId(row.id),
            CompiledAbility {
                id: AbilityId(row.id),
                name: row.name.clone(),
                max_level,
                definition: Arc::new(definition),
                targeting: Arc::clone(target),
            },
        );
    }
    Ok(abilities)
}

fn compile_additional_costs(
    prepared: &PreparedTables,
    ability_id: i32,
    resources: &BTreeMap<&str, UniqueName>,
) -> Result<Vec<AdditionalCost>, ConfigError> {
    prepared
        .additional_costs(ability_id)
        .iter()
        .map(|cost| {
            let location = ConfigLocation::table("AbilityAdditionalCost").row(cost.row.id);
            let resource = resources
                .get(cost.row.resource.as_str())
                .copied()
                .ok_or_else(|| {
                    ConfigError::new(
                        ConfigErrorKind::Reference,
                        location.field("resource"),
                        "unresolved registered resource",
                    )
                })?;
            AdditionalCost::new(resource, cost.amount).map_err(|error| {
                ConfigError::new(
                    ConfigErrorKind::InvalidValue,
                    location.field("amount"),
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn compile_tasks(
    prepared: &PreparedTables,
    ability_id: i32,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
) -> Result<Vec<AbilityTaskDef>, ConfigError> {
    let mut groups: BTreeMap<u32, Vec<AbilityTaskOnFinishedDef>> = BTreeMap::new();
    for row in prepared.actions(ability_id) {
        let action = match row.kind {
            PreparedActionKind::EndAbility => AbilityTaskOnFinishedDef::EndAbility,
            PreparedActionKind::ApplyEffect { effect_id, scope } => {
                let effect = resolve_effect(effect_id, effects)?;
                match scope {
                    PreparedTargetScope::Primary => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect }
                    }
                    PreparedTargetScope::AllCaptured => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect }
                    }
                }
            }
        };
        groups.entry(row.tick).or_default().push(action);
    }
    Ok(groups
        .into_iter()
        .map(|(tick, actions)| {
            let batch = AbilityTaskOnFinishedDef::Batch { actions };
            if tick == 0 {
                AbilityTaskDef::instant(batch)
            } else {
                AbilityTaskDef::wait_ticks(tick, batch)
            }
        })
        .collect())
}
