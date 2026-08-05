use super::super::commit::can_pay_ability_cost;
use super::super::params::AbilitySystemParams;
use crate::gameplay_abilities::GameplayAbility;
use bevy::prelude::Entity;
use std::sync::Arc;

/// Returns whether an ability currently satisfies activation and cost requirements.
pub fn can_activate_ability(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    if !passes_ability_activation_requirements(source, ability, params) {
        return false;
    }

    can_pay_ability_cost(source, target, ability, level, params)
}

pub(super) fn passes_ability_activation_requirements(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    params: &mut AbilitySystemParams,
) -> bool {
    if let Ok(asc) = params.asc_query.get(source)
        && asc
            .get_blocked_ability_tags()
            .has_any(ability.get_tags().get_ability_asset_tags())
    {
        return false;
    }

    let source_tags = params.effects.tag_container_query.get(source).ok();
    if let Some(tags) = source_tags {
        let ability_tags = ability.get_tags();
        if tags.has_any(ability_tags.get_activation_blocked_tags()) {
            return false;
        }
        if !tags.has_all(ability_tags.get_activation_required_tags()) {
            return false;
        }

        if let Some(cooldown_def) = ability.get_cooldown()
            && tags.has_any(cooldown_def.get_tags().get_granted_tags())
        {
            return false;
        }
    }

    true
}
