use super::super::gameplay_effect::{EffectPayload, GameplayEffect, StackingType};
use super::super::gameplay_effect_spec::GameplayEffectSpec;
use super::super::{EffectSystemParams, EffectTags};
use super::execution::execute_gameplay_effect_plan_in_batch;
use super::planning::{GameplayEffectApplicationError, prepare_gameplay_effect};
use super::requirements::{
    resolve_active_effect_tag_requirements, resolve_active_effect_tag_requirements_if_dirty,
};
use super::state::{ActiveEffectHandle, ActiveGameplayEffects};
use crate::gameplay_tags::{GameplayTagError, tag_bits_from_tags_with_manager};
use bevy::prelude::*;
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

pub(super) fn find_stackable_active_effect(
    source: Entity,
    target: Entity,
    spec: &GameplayEffectSpec,
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
    ignored_handles: &[ActiveEffectHandle],
) -> Option<(ActiveEffectHandle, u32)> {
    let stacking_type = spec.get_stacking_policy().get_stacking_type();
    if matches!(stacking_type, StackingType::None) {
        return None;
    }
    let active_effects = active_effect_query.get(target).ok()?;
    active_effects.handles(target).find_map(|handle| {
        if ignored_handles.contains(&handle) {
            return None;
        }
        let effect = active_effects.get(handle)?;
        (spec.is_same_def(effect.get_spec())
            && match stacking_type {
                StackingType::None => false,
                StackingType::AggregateBySource => effect.get_source() == source,
                StackingType::AggregateByTarget => true,
            })
        .then_some((handle, effect.get_stack_count()))
    })
}

pub(super) fn passes_application_requirements(
    source: Entity,
    target: Entity,
    incoming_tags: &EffectTags,
    params: &EffectSystemParams,
) -> bool {
    let source_tags = params.tag_container_query.get(source).ok();
    let target_tags = params.tag_container_query.get(target).ok();
    incoming_tags
        .get_source_application_tags()
        .passes(source_tags)
        && incoming_tags
            .get_target_application_tags()
            .passes(target_tags)
}

pub(super) fn is_blocked_by_application_immunity(
    source: Entity,
    target: Entity,
    incoming_tags: &EffectTags,
    params: &mut EffectSystemParams,
) -> Result<bool, GameplayTagError> {
    let source_tags = params.tag_container_query.get(source).ok();
    let incoming_asset_bits =
        tag_bits_from_tags_with_manager(incoming_tags.get_asset_tags(), &params.tag_manager)?;
    let Ok(active_effects) = params.active_effect_query.get(target) else {
        return Ok(false);
    };
    for handle in active_effects.handles(target) {
        let Some(effect) = active_effects.get(handle) else {
            continue;
        };
        if effect.is_inhibited() {
            continue;
        }
        if effect
            .get_spec()
            .get_def_tags()
            .get_granted_application_immunity()
            .iter()
            .any(|immunity| immunity.matches_tag_bits(source_tags, Some(&incoming_asset_bits)))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
