//! Commits prepared effects and compensates temporary external payments on failure.

use super::super::params::AbilitySystemParams;
use super::additional_cost::AdditionalCostProvider;
use super::affordability::can_pay_cost_plan;
use super::error::AbilityCommitError;
use super::planning::{AbilityCommitPlans, prepare_ability_commit_plans};
use crate::gameplay_abilities::GameplayAbility;
use crate::gameplay_effects::{
    EffectSystemParams, GameplayEffectApplicationPlan, execute_gameplay_effect_plan_in_batch,
    validate_gameplay_effect_plan,
};
use bevy::prelude::Entity;
use std::sync::Arc;

/// Applies an ability's cost and cooldown without starting an ability instance.
///
/// Costs are previewed in modifier order using their base operations and aggregators after any
/// planned effect removals. Each resulting current value must be finite and nonnegative; cost
/// magnitudes and resulting base values must also be finite. Post-execute callbacks run only
/// during actual execution and their mutations are outside the affordability preview.
/// Additional costs use provider `P`: external resources are temporarily debited before effect
/// execution and compensated on failure. Successful commit confirms them before startup; later
/// cancellation does not refund. Compensation does not roll back earlier GAS effect mutations.
///
/// # Errors
///
/// Returns [`AbilityCommitError`] when the cost or cooldown cannot be prepared,
/// paid, or executed.
pub fn commit_ability<P: AdditionalCostProvider>(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams<'_, '_, P>,
) -> Result<(), AbilityCommitError> {
    let plans = prepare_ability_commit_plans(source, ability, level, None, params)?;
    execute_ability_commit_plans(plans, params)
}

pub(in super::super) fn execute_ability_commit_plans<P: AdditionalCostProvider>(
    plans: AbilityCommitPlans<'_>,
    params: &mut AbilitySystemParams<'_, '_, P>,
) -> Result<(), AbilityCommitError> {
    if let Some(plan) = plans.cost_plan.as_ref() {
        validate_gameplay_effect_plan(plan, &mut params.effects)
            .map_err(AbilityCommitError::CostExecution)?;
    }
    if let Some(plan) = plans.cooldown_plan.as_ref() {
        validate_gameplay_effect_plan(plan, &mut params.effects)
            .map_err(AbilityCommitError::CooldownExecution)?;
    }

    if let Some(plan) = plans.cost_plan.as_ref()
        && !can_pay_cost_plan(plan, &params.effects).map_err(AbilityCommitError::CostExecution)?
    {
        return Err(AbilityCommitError::InsufficientCost);
    }

    // Keep compensation local to this call; preparation never leaves a pending reservation
    // across cancellation, instance creation, deferred commands, or fixed-tick boundaries.
    let payment = if let Some(additional) = plans.additional_cost_plan {
        let receipt = P::prepare(
            &mut params.additional_costs,
            &additional.context,
            additional.costs,
        )
        .map_err(AbilityCommitError::AdditionalCost)?;
        Some((additional.context, receipt))
    } else {
        None
    };

    let result = execute_effect_plans(plans.cost_plan, plans.cooldown_plan, &mut params.effects);

    if let Err(commit_error) = result {
        if let Some((context, receipt)) = payment
            && let Err(rollback_error) =
                P::rollback(&mut params.additional_costs, &context, receipt)
        {
            return Err(AbilityCommitError::AdditionalCostRollback {
                commit_error: Box::new(commit_error),
                rollback_error,
            });
        }
        return Err(commit_error);
    }

    // A successful payment is confirmed by dropping the receipt, before any startup actions.
    Ok(())
}

fn execute_effect_plans(
    cost_plan: Option<GameplayEffectApplicationPlan>,
    cooldown_plan: Option<GameplayEffectApplicationPlan>,
    params: &mut EffectSystemParams,
) -> Result<(), AbilityCommitError> {
    if let Some(plan) = cost_plan {
        execute_gameplay_effect_plan_in_batch(plan, params)
            .map_err(AbilityCommitError::CostExecution)?;
    }
    if let Some(plan) = cooldown_plan {
        execute_gameplay_effect_plan_in_batch(plan, params)
            .map_err(AbilityCommitError::CooldownExecution)?;
    }
    Ok(())
}
