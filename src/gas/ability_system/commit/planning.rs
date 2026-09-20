//! Prepares effect plans and checks external costs without retaining a payment receipt.

use super::super::params::AbilitySystemParams;
use super::additional_cost::{AdditionalCostContext, AdditionalCostProvider};
use super::affordability::can_pay_cost_plan;
use super::error::AbilityCommitError;
use crate::gameplay_abilities::{
    AbilityActivationContext, AdditionalCost, GameplayAbility, effect_payload_from_ability_context,
};
use crate::gameplay_effects::{GameplayEffectApplicationPlan, prepare_gameplay_effect};
use bevy::prelude::Entity;
use std::sync::Arc;

pub(in super::super) struct AbilityCommitPlans<'a> {
    pub(super) cost_plan: Option<GameplayEffectApplicationPlan>,
    pub(super) cooldown_plan: Option<GameplayEffectApplicationPlan>,
    pub(super) additional_cost_plan: Option<AdditionalCostPlan<'a>>,
}

pub(super) struct AdditionalCostPlan<'a> {
    pub(super) context: AdditionalCostContext<'a>,
    pub(super) costs: &'a [AdditionalCost],
}

pub(in super::super) fn prepare_ability_commit_plans<'a, P: AdditionalCostProvider>(
    source: Entity,
    ability: &'a Arc<GameplayAbility>,
    level: u32,
    activation_context: Option<&'a AbilityActivationContext>,
    params: &mut AbilitySystemParams<'_, '_, P>,
) -> Result<AbilityCommitPlans<'a>, AbilityCommitError> {
    let additional_cost_plan = if ability.get_additional_costs().is_empty() {
        None
    } else {
        let context = AdditionalCostContext {
            source,
            level,
            activation: activation_context,
        };
        let costs = ability.get_additional_costs();
        P::check(&params.additional_costs, &context, costs)
            .map_err(AbilityCommitError::AdditionalCost)?;
        Some(AdditionalCostPlan { context, costs })
    };
    let cost_plan = if let Some(cost_def) = ability.get_cost() {
        if !cost_def.has_only_add_modifiers() {
            return Err(AbilityCommitError::CostModifiersMustBeAdditive);
        }
        let payload = effect_payload_from_ability_context(source, level, activation_context);
        let plan = prepare_gameplay_effect(source, cost_def, &mut params.effects, &payload)
            .map_err(AbilityCommitError::CostPreparation)?;
        if !plan.is_instant() {
            return Err(AbilityCommitError::CostMustBeInstant);
        }
        if !can_pay_cost_plan(&plan, &params.effects)
            .map_err(AbilityCommitError::CostPreparation)?
        {
            return Err(AbilityCommitError::InsufficientCost);
        }
        Some(plan)
    } else {
        None
    };

    let cooldown_plan = if let Some(cooldown_def) = ability.get_cooldown() {
        let payload = effect_payload_from_ability_context(source, level, activation_context);
        let plan = prepare_gameplay_effect(source, cooldown_def, &mut params.effects, &payload)
            .map_err(AbilityCommitError::CooldownPreparation)?;
        Some(plan)
    } else {
        None
    };

    Ok(AbilityCommitPlans {
        cost_plan,
        cooldown_plan,
        additional_cost_plan,
    })
}
