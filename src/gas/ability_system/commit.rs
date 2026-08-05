use super::params::AbilitySystemParams;
use crate::gameplay_abilities::{AbilityActivationContext, GameplayAbility};
use crate::gameplay_effects::{
    EffectContext, EffectPayload, GameplayEffectApplicationError, GameplayEffectApplicationPlan,
    execute_gameplay_effect_plan_in_batch, prepare_gameplay_effect, validate_gameplay_effect_plan,
};
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
    /// The source does not have enough initialized attribute value.
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
        let payload =
            effect_payload_from_optional_activation_context(source, level, activation_context);
        let plan = prepare_gameplay_effect(source, cost_def, &mut params.effects, &payload)
            .map_err(AbilityCommitError::CostPreparation)?;
        if !plan.is_instant() {
            return Err(AbilityCommitError::CostMustBeInstant);
        }
        if !can_pay_prepared_cost(source, &plan, params) {
            return Err(AbilityCommitError::InsufficientCost);
        }
        Some(plan)
    } else {
        None
    };

    let cooldown_plan = if let Some(cooldown_def) = ability.get_cooldown() {
        let payload =
            effect_payload_from_optional_activation_context(source, level, activation_context);
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

fn effect_payload_from_optional_activation_context(
    source: Entity,
    level: u32,
    activation_context: Option<&AbilityActivationContext>,
) -> EffectPayload {
    activation_context.map_or_else(
        || EffectPayload::new(source, None, level),
        |activation_context| {
            effect_payload_from_activation_context(source, level, activation_context)
        },
    )
}

pub(super) fn effect_payload_from_activation_context(
    source: Entity,
    level: u32,
    activation_context: &AbilityActivationContext,
) -> EffectPayload {
    let payload = EffectPayload::new(source, activation_context.get_causer(), level)
        .with_instigator(activation_context.get_instigator());
    if let Some(source_snapshot) = activation_context.get_source_snapshot() {
        payload.with_source_snapshot(source_snapshot.clone())
    } else {
        payload
    }
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
        execute_gameplay_effect_plan_in_batch(plan, &mut params.effects)
            .map_err(AbilityCommitError::CostExecution)?;
    }

    if let Some(plan) = plans.cooldown_plan {
        execute_gameplay_effect_plan_in_batch(plan, &mut params.effects)
            .map_err(AbilityCommitError::CooldownExecution)?;
    }

    Ok(())
}

fn can_pay_prepared_cost(
    source: Entity,
    cost_plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> bool {
    let Ok(mut attr_set) = params.effects.attr_set_query.get_mut(source) else {
        return false;
    };

    for cost in cost_plan.get_modifier_specs() {
        let Ok(Some(current_val)) =
            attr_set.get_current_value(&params.effects.attribute_id_manager, cost.get_id())
        else {
            return false;
        };
        if current_val + cost.get_value() < 0.0 {
            return false;
        }
    }

    true
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

    let payload = EffectPayload::new(source, None, level);
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

    let Ok(mut attr_set) = params.effects.attr_set_query.get_mut(source) else {
        return false;
    };

    for cost in cost_spec.get_modifier_specs() {
        let Ok(Some(current_val)) =
            attr_set.get_current_value(&params.effects.attribute_id_manager, cost.get_id())
        else {
            return false;
        };
        if current_val + cost.get_value() < 0.0 {
            return false;
        }
    }

    true
}
