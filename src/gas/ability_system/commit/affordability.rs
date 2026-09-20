//! Shared attribute-cost previews for advisory checks and authoritative payment.

use super::error::AbilityCommitError;
use crate::attributes::AttributeSnapshot;
use crate::gameplay_abilities::{GameplayAbility, effect_payload_from_ability_context};
use crate::gameplay_effects::{
    EffectContext, EffectReadOnlyParams, EffectSystemParams, GameplayEffectApplicationError,
    GameplayEffectApplicationPlan, preview_instant_effect_modifiers,
};
use crate::modifiers::ModifierSpec;
use bevy::prelude::Entity;
use std::sync::Arc;

fn cost_modifiers_are_finite(modifiers: &[ModifierSpec]) -> bool {
    modifiers
        .iter()
        .all(|modifier| modifier.get_value().is_finite())
}

fn cost_balance_is_payable(value: AttributeSnapshot) -> bool {
    value.base().is_finite() && value.current().is_finite() && value.current() >= 0.0
}

pub(super) fn can_pay_cost_plan(
    plan: &GameplayEffectApplicationPlan,
    params: &EffectSystemParams,
) -> Result<bool, GameplayEffectApplicationError> {
    if !cost_modifiers_are_finite(plan.get_modifier_specs()) {
        return Ok(false);
    }
    plan.preview_modifier_application(params, cost_balance_is_payable)
}

pub(in super::super) fn check_ability_cost(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &EffectReadOnlyParams,
) -> Result<(), AbilityCommitError> {
    let Some(cost_def) = ability.get_cost() else {
        return Ok(());
    };
    if !cost_def.has_only_add_modifiers() {
        return Err(AbilityCommitError::CostModifiersMustBeAdditive);
    }

    let payload = effect_payload_from_ability_context(source, level, None);
    let cost_spec = {
        let context = EffectContext {
            target: Some(target),
            payload: &payload,
            attribute_id_manager: &params.attribute_id_manager,
            attr_set_query: &params.attr_set_query,
            tag_container_query: &params.tag_container_query,
        };

        cost_def.make_spec(&context)
    };

    if !cost_modifiers_are_finite(cost_spec.get_modifier_specs()) {
        return Err(AbilityCommitError::InsufficientCost);
    }
    let payable = preview_instant_effect_modifiers(
        source,
        &cost_spec,
        &params.attr_set_query,
        &params.active_effect_query,
        &params.attribute_id_manager,
        &params.tag_manager,
        cost_balance_is_payable,
    )
    .map_err(AbilityCommitError::CostPreparation)?;
    if !payable {
        return Err(AbilityCommitError::InsufficientCost);
    }
    Ok(())
}
