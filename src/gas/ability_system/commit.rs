use super::params::AbilitySystemParams;
use crate::attributes::AttributeSnapshot;
use crate::gameplay_abilities::{
    AbilityActivationContext, GameplayAbility, effect_payload_from_ability_context,
};
use crate::gameplay_effects::{
    EffectContext, GameplayEffectApplicationError, GameplayEffectApplicationPlan,
    execute_gameplay_effect_plan_in_batch, prepare_gameplay_effect,
    preview_instant_effect_modifiers, validate_gameplay_effect_plan,
};
use crate::modifiers::ModifierSpec;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Describes why an ability cost or cooldown could not be committed.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityCommitError {
    /// A cost definition contains an operation other than addition.
    CostModifiersMustBeAdditive,
    /// The cost effect could not be prepared.
    CostPreparation(GameplayEffectApplicationError),
    /// A cost definition is not instant.
    CostMustBeInstant,
    /// A cost magnitude or projected base is non-finite, or projected current is non-finite or negative.
    InsufficientCost,
    /// The cooldown effect could not be prepared.
    CooldownPreparation(GameplayEffectApplicationError),
    /// The prepared cost plan could not be executed.
    CostExecution(GameplayEffectApplicationError),
    /// The prepared cooldown plan could not be executed.
    CooldownExecution(GameplayEffectApplicationError),
}

impl fmt::Display for AbilityCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CostModifiersMustBeAdditive => {
                write!(f, "ability cost may contain only additive modifiers")
            }
            Self::CostPreparation(error) => write!(f, "ability cost preparation failed: {error}"),
            Self::CostMustBeInstant => write!(f, "ability cost must be an instant effect"),
            Self::InsufficientCost => write!(f, "ability source cannot pay the prepared cost"),
            Self::CooldownPreparation(error) => {
                write!(f, "ability cooldown preparation failed: {error}")
            }
            Self::CostExecution(error) => write!(f, "ability cost execution failed: {error}"),
            Self::CooldownExecution(error) => {
                write!(f, "ability cooldown execution failed: {error}")
            }
        }
    }
}

impl Error for AbilityCommitError {}

impl AbilityCommitError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        match self {
            Self::CostPreparation(error) | Self::CooldownPreparation(error) => error.is_rejection(),
            Self::InsufficientCost => true,
            Self::CostModifiersMustBeAdditive
            | Self::CostMustBeInstant
            | Self::CostExecution(_)
            | Self::CooldownExecution(_) => false,
        }
    }
}

/// Applies an ability's cost and cooldown without starting an ability instance.
///
/// Costs are previewed in modifier order using their base operations and aggregators after any
/// planned effect removals. Each resulting current value must be finite and nonnegative; cost
/// magnitudes and resulting base values must also be finite. Post-execute callbacks run only
/// during actual execution and their mutations are outside the affordability preview.
///
/// # Errors
///
/// Returns [`AbilityCommitError`] when the cost or cooldown cannot be prepared,
/// paid, or executed.
pub fn commit_ability(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityCommitError> {
    let plans = prepare_ability_commit_plans(source, ability, level, None, params)?;
    execute_ability_commit_plans(plans, params)
}

pub(super) struct AbilityCommitPlans {
    cost_plan: Option<GameplayEffectApplicationPlan>,
    cooldown_plan: Option<GameplayEffectApplicationPlan>,
}

pub(super) fn prepare_ability_commit_plans(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    activation_context: Option<&AbilityActivationContext>,
    params: &mut AbilitySystemParams,
) -> Result<AbilityCommitPlans, AbilityCommitError> {
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
        if !can_pay_cost_plan(&plan, params).map_err(AbilityCommitError::CostPreparation)? {
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
    })
}

pub(super) fn execute_ability_commit_plans(
    plans: AbilityCommitPlans,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityCommitError> {
    if let Some(plan) = plans.cost_plan.as_ref() {
        validate_gameplay_effect_plan(plan, &mut params.effects)
            .map_err(AbilityCommitError::CostExecution)?;
    }
    if let Some(plan) = plans.cooldown_plan.as_ref() {
        validate_gameplay_effect_plan(plan, &mut params.effects)
            .map_err(AbilityCommitError::CooldownExecution)?;
    }

    if let Some(plan) = plans.cost_plan {
        if !can_pay_cost_plan(&plan, params).map_err(AbilityCommitError::CostExecution)? {
            return Err(AbilityCommitError::InsufficientCost);
        }
        execute_gameplay_effect_plan_in_batch(plan, &mut params.effects)
            .map_err(AbilityCommitError::CostExecution)?;
    }

    if let Some(plan) = plans.cooldown_plan {
        execute_gameplay_effect_plan_in_batch(plan, &mut params.effects)
            .map_err(AbilityCommitError::CooldownExecution)?;
    }

    Ok(())
}

fn cost_modifiers_are_finite(modifiers: &[ModifierSpec]) -> bool {
    modifiers
        .iter()
        .all(|modifier| modifier.get_value().is_finite())
}

fn cost_balance_is_payable(value: AttributeSnapshot) -> bool {
    value.base().is_finite() && value.current().is_finite() && value.current() >= 0.0
}

fn can_pay_cost_plan(
    plan: &GameplayEffectApplicationPlan,
    params: &AbilitySystemParams,
) -> Result<bool, GameplayEffectApplicationError> {
    if !cost_modifiers_are_finite(plan.get_modifier_specs()) {
        return Ok(false);
    }
    plan.preview_modifier_application(&params.effects, cost_balance_is_payable)
}

pub(super) fn can_pay_ability_cost(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    let Some(cost_def) = ability.get_cost() else {
        return true;
    };
    if !cost_def.has_only_add_modifiers() {
        return false;
    }

    let payload = effect_payload_from_ability_context(source, level, None);
    let cost_spec = {
        let context = EffectContext {
            target: Some(target),
            payload: &payload,
            attribute_id_manager: &params.effects.attribute_id_manager,
            attr_set_query: &params.effects.attr_set_query.as_readonly(),
            tag_container_query: &params.effects.tag_container_query.as_readonly(),
        };

        cost_def.make_spec(&context)
    };

    cost_modifiers_are_finite(cost_spec.get_modifier_specs())
        && preview_instant_effect_modifiers(
            source,
            &cost_spec,
            &params.effects,
            cost_balance_is_payable,
        )
        .is_ok_and(|payable| payable)
}
