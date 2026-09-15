//! Ability definitions and their ordered startup task timelines.

use super::effects::resolve_effect;
use super::registration::resolve_tags;
use super::{
    AbilityId, CompiledAbility, ConfigError, ConfigErrorKind, ConfigLocation, EffectId, Tables,
    data,
};
use crate::{
    AbilityTags, AbilityTaskDef, AbilityTaskOnFinishedDef, GameplayAbility, GameplayEffect,
    GameplayTag, TargetingDefinition,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn compile_abilities(
    tables: &Tables,
    tags: &BTreeMap<String, GameplayTag>,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
    targeting: &BTreeMap<i32, Arc<TargetingDefinition>>,
) -> Result<BTreeMap<AbilityId, CompiledAbility>, ConfigError> {
    let mut abilities = BTreeMap::new();
    for row in tables.tb_ability.iter() {
        let ability_tags = AbilityTags::default()
            .with_ability_asset_tags(resolve_tags(&row.asset_tags, tags)?)
            .with_activation_required_tags(resolve_tags(&row.required_tags, tags)?)
            .with_activation_blocked_tags(resolve_tags(&row.blocked_tags, tags)?)
            .with_cancel_abilities_with_tags(resolve_tags(&row.cancel_ability_tags, tags)?)
            .with_block_abilities_with_tags(resolve_tags(&row.block_ability_tags, tags)?);
        let activation_effects = row
            .activation_effect_ids
            .iter()
            .map(|id| resolve_effect(*id, effects))
            .collect::<Result<Vec<_>, _>>()?;
        let mut definition = GameplayAbility::default()
            .with_tags(ability_tags)
            .with_startup_tasks(compile_tasks(tables, row.id, effects)?)
            .with_activation_effects(activation_effects)
            .with_end_on_activation(row.end_on_activation)
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

fn compile_tasks(
    tables: &Tables,
    ability_id: i32,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
) -> Result<Vec<AbilityTaskDef>, ConfigError> {
    let mut rows: Vec<_> = tables
        .tb_ability_action
        .iter()
        .filter(|action| action.ability_id == ability_id)
        .collect();
    rows.sort_by_key(|action| (action.at_tick, action.order));
    let mut groups: BTreeMap<u32, Vec<AbilityTaskOnFinishedDef>> = BTreeMap::new();
    for row in rows {
        let context = ConfigLocation::table("AbilityAction").row(row.id);
        let action = match row.kind {
            data::ActionKind::EndAbility => AbilityTaskOnFinishedDef::EndAbility,
            data::ActionKind::ApplyEffect => {
                let effect = resolve_effect(
                    row.effect_id.ok_or_else(|| {
                        ConfigError::new(
                            ConfigErrorKind::Reference,
                            context.field("effect_id"),
                            "missing effect reference",
                        )
                    })?,
                    effects,
                )?;
                match row.target_scope {
                    data::TargetScope::Primary => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect }
                    }
                    data::TargetScope::AllCaptured => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect }
                    }
                    data::TargetScope::None => {
                        return Err(ConfigError::new(
                            ConfigErrorKind::InvalidValue,
                            context.field("target_scope"),
                            "ApplyEffect needs a target scope",
                        ));
                    }
                }
            }
        };
        let tick = u32::try_from(row.at_tick).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::InvalidValue,
                context.field("at_tick"),
                error.to_string(),
            )
        })?;
        groups.entry(tick).or_default().push(action);
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
