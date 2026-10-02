use super::super::EffectSystemParams;
use super::super::gameplay_effect::{EffectPayload, GameplayEffect};
use super::error::GameplayEffectApplicationError;
use super::execution::execute_gameplay_effect_plan_in_batch;
use super::planning::prepare_gameplay_effect;
use super::requirements::{
    resolve_active_effect_tag_requirements, resolve_active_effect_tag_requirements_if_dirty,
};
use bevy::prelude::Entity;
use std::sync::Arc;

/// Prepares and executes a gameplay-effect application through an independent synchronous call path.
///
/// Active-effect tag requirements are converged before preparation and again before this function
/// returns. Runtime producer systems should enqueue applications when ordering against the global
/// [`GameplayExecutionQueue`](crate::GameplayExecutionQueue) matters. This call does not provide
/// transactional rollback if execution fails after earlier gameplay mutations.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] for rejection or invalid runtime state.
pub fn apply_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let result = apply_gameplay_effect_in_batch(target, effect_def, params, payload);
    resolve_active_effect_tag_requirements_if_dirty(params);
    result
}

pub(crate) fn apply_gameplay_effect_in_batch(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError> {
    let plan = prepare_gameplay_effect(target, effect_def, params, payload)?;
    execute_gameplay_effect_plan_in_batch(plan, params)
}
