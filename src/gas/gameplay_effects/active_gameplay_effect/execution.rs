use super::super::EffectSystemParams;
use super::super::gameplay_effect::{StackDurationPolicy, StackMagnitudePolicy, StackPeriodPolicy};
use super::super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use super::planning::{
    GameplayEffectApplicationError, GameplayEffectApplicationKind, GameplayEffectApplicationPlan,
    map_attribute_set_error,
};
use super::removal::{
    EffectCleanupResources, remove_collected_active_effects_for_params, validate_effect_cleanup,
};
use super::requirements::resolve_active_effect_tag_requirements_if_dirty;
use super::state::{ActiveEffectHandle, ActiveGameplayEffect, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{
    GameplayTagContainer, GameplayTagManager, tag_bits_from_tags_with_manager,
};
use bevy::prelude::*;

/// Executes an immediately prepared gameplay-effect plan.
///
/// This revalidates structural ECS requirements, but does not repeat application
/// requirements, immunity, probability, or stacking decisions. It also does not
/// provide transactional rollback if a later mutation fails.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if required ECS state changed.
pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements_if_dirty(params);
    let result = execute_gameplay_effect_plan_in_batch(plan, params);
    resolve_active_effect_tag_requirements_if_dirty(params);
    result
}

pub(crate) fn execute_gameplay_effect_plan_in_batch(
    plan: GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    validate_gameplay_effect_plan(&plan, params)?;
    if plan.changes_active_effect_requirements() {
        params.active_effect_requirement_sync.mark_dirty();
    }
    remove_collected_active_effects_for_params(&plan.removed_effects, params)?;
    match plan.kind {
        GameplayEffectApplicationKind::Instant => execute_instant_effect(&plan, params),
        GameplayEffectApplicationKind::StackExisting {
            handle,
            new_stack_count,
        } => execute_stack_existing_effect(&plan, handle, new_stack_count, params),
        GameplayEffectApplicationKind::CreateActive => execute_new_active_effect(&plan, params),
    }
}

pub(crate) fn validate_gameplay_effect_plan(
    plan: &GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(active_effects) = params.active_effect_query.get(plan.target) {
        for &handle in &plan.removed_effects {
            if let Some(effect) = active_effects.get(handle) {
                validate_effect_cleanup(
                    effect,
                    EffectCleanupResources {
                        attribute_id_manager: &params.attribute_id_manager,
                        tag_manager: &params.tag_manager,
                    },
                )?;
            }
        }
    }
    validate_effect_execution_requirements(
        plan.target,
        &plan.spec,
        &params.attribute_id_manager,
        &params.tag_manager,
        &params.attr_set_query,
        &params.tag_container_query,
        &params.active_effect_query,
    )?;
    Ok(())
}

pub(super) fn validate_effect_execution_requirements(
    target: Entity,
    spec: &GameplayEffectSpec,
    attribute_id_manager: &AttributeIdManager,
    tag_manager: &Res<GameplayTagManager>,
    attr_query: &Query<&mut AttributeSet>,
    tag_query: &Query<&mut GameplayTagContainer>,
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
) -> Result<(), GameplayEffectApplicationError> {
    if !spec.get_duration_spec().is_instant() && active_effect_query.get(target).is_err() {
        return Err(GameplayEffectApplicationError::MissingActiveGameplayEffects { target });
    }
    if !spec.get_modifier_specs().is_empty() {
        let Ok(attr_set) = attr_query.get(target) else {
            return Err(GameplayEffectApplicationError::MissingAttributeSet { target });
        };
        for modifier in spec.get_modifier_specs() {
            attr_set
                .validate_initialized_attribute(attribute_id_manager, modifier.get_id())
                .map_err(|error| map_attribute_set_error(target, error))?;
        }
    }
    if !spec.get_def_tags().get_granted_tags().is_empty() && tag_query.get(target).is_err() {
        return Err(GameplayEffectApplicationError::MissingTagContainer { target });
    }
    tag_bits_from_tags_with_manager(spec.get_def_tags().get_granted_tags(), tag_manager)?;
    Ok(())
}

fn execute_instant_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if plan.spec.get_modifier_specs().is_empty() {
        return Ok(());
    }

    let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
        return Err(GameplayEffectApplicationError::MissingAttributeSet {
            target: plan.target,
        });
    };
    apply_instant_modifiers(
        plan.target,
        &mut attributes,
        &params.attribute_id_manager,
        &plan.spec,
        1,
    )
}

fn execute_stack_existing_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    new_stack_count: u32,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let (target, spec, inhibited, runtime_periodic) = {
        let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) else {
            return execute_new_active_effect(plan, params);
        };
        let Some(effect) = active_effects.get_mut(handle) else {
            return execute_new_active_effect(plan, params);
        };
        effect.set_stack_count(new_stack_count);
        if matches!(
            plan.spec.get_stacking_policy().get_duration_policy(),
            StackDurationPolicy::RefreshOnSuccessfulStack
        ) && let (EffectDurationTicksSpec::DurationTicks(duration_ticks), Some(duration)) =
            (plan.spec.get_duration_spec(), effect.duration.as_mut())
        {
            duration.remain_ticks = *duration_ticks;
        }
        if matches!(
            plan.spec.get_stacking_policy().get_period_policy(),
            StackPeriodPolicy::ResetOnSuccessfulStack
        ) && let Some(period) = effect.period.as_mut()
        {
            period.current_tick = 0;
        }
        (
            effect.get_target(),
            effect.get_spec().clone(),
            effect.is_inhibited(),
            effect.period.is_some(),
        )
    };

    if !inhibited && !runtime_periodic && !spec.get_modifier_specs().is_empty() {
        let Ok(mut attributes) = params.attr_set_query.get_mut(target) else {
            return Err(GameplayEffectApplicationError::MissingAttributeSet { target });
        };
        attributes.remove_modifiers_for_attributes(
            &params.attribute_id_manager,
            handle,
            spec.get_modified_attribute_ids(),
        )?;
        apply_duration_modifiers(
            target,
            &mut attributes,
            &params.attribute_id_manager,
            &spec,
            handle,
            new_stack_count,
        )?;
    }
    Ok(())
}

fn execute_new_active_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let has_modifiers = !plan.spec.get_modifier_specs().is_empty();
    let grants_tags = !plan.spec.get_def_tags().get_granted_tags().is_empty();
    let handle = {
        let Ok(mut active_effects) = params.active_effect_query.get_mut(plan.target) else {
            return Err(
                GameplayEffectApplicationError::MissingActiveGameplayEffects {
                    target: plan.target,
                },
            );
        };
        active_effects.insert(
            plan.target,
            ActiveGameplayEffect::new(plan.spec.clone(), plan.source, plan.target),
        )?
    };

    if grants_tags {
        let Ok(mut tags) = params.tag_container_query.get_mut(plan.target) else {
            rollback_new_active_effect(plan, handle, false, params)?;
            return Err(GameplayEffectApplicationError::MissingTagContainer {
                target: plan.target,
            });
        };
        if let Err(error) = tags.add_tags(
            plan.spec.get_def_tags().get_granted_tags(),
            &params.tag_manager,
        ) {
            rollback_new_active_effect(plan, handle, false, params)?;
            return Err(error.into());
        }
    }

    if let Some(period_spec) = plan.spec.get_period_spec() {
        if period_spec.get_period_ticks() == 0 {
            if has_modifiers {
                let result = apply_new_duration_modifiers(plan, handle, params);
                if let Err(error) = result {
                    rollback_new_active_effect(plan, handle, grants_tags, params)?;
                    return Err(error);
                }
            }
        } else if period_spec.get_execute_on_applied() && has_modifiers {
            let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
                rollback_new_active_effect(plan, handle, grants_tags, params)?;
                return Err(GameplayEffectApplicationError::MissingAttributeSet {
                    target: plan.target,
                });
            };
            if let Err(error) = apply_instant_modifiers(
                plan.target,
                &mut attributes,
                &params.attribute_id_manager,
                &plan.spec,
                1,
            ) {
                rollback_new_active_effect(plan, handle, grants_tags, params)?;
                return Err(error);
            }
        }
    } else if has_modifiers {
        let result = apply_new_duration_modifiers(plan, handle, params);
        if let Err(error) = result {
            rollback_new_active_effect(plan, handle, grants_tags, params)?;
            return Err(error);
        }
    }
    Ok(())
}

fn apply_new_duration_modifiers(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
        return Err(GameplayEffectApplicationError::MissingAttributeSet {
            target: plan.target,
        });
    };
    apply_duration_modifiers(
        plan.target,
        &mut attributes,
        &params.attribute_id_manager,
        &plan.spec,
        handle,
        1,
    )
}

fn rollback_new_active_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    tags_applied: bool,
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) {
        attributes.remove_modifiers(handle);
    }
    let tag_result = if tags_applied {
        match params.tag_container_query.get_mut(plan.target) {
            Ok(mut tags) => tags
                .remove_tags(
                    plan.spec.get_def_tags().get_granted_tags(),
                    &params.tag_manager,
                )
                .map_err(GameplayEffectApplicationError::from),
            Err(_) => Err(GameplayEffectApplicationError::MissingTagContainer {
                target: plan.target,
            }),
        }
    } else {
        Ok(())
    };
    if let Ok(mut active_effects) = params.active_effect_query.get_mut(plan.target) {
        active_effects.remove(handle);
    }
    tag_result
}

pub(super) fn apply_duration_modifiers(
    target: Entity,
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    handle: ActiveEffectHandle,
    stack_count: u32,
) -> Result<(), GameplayEffectApplicationError> {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for modifier in spec.get_modifier_specs() {
        let stacked = modifier.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_duration_modifier(attribute_id_manager, &stacked, handle)
            .map_err(|error| map_attribute_set_error(target, error))?;
    }
    Ok(())
}

pub(super) fn apply_instant_modifiers(
    target: Entity,
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    stack_count: u32,
) -> Result<(), GameplayEffectApplicationError> {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for modifier in spec.get_modifier_specs() {
        let stacked = modifier.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_instant_modifier(attribute_id_manager, &stacked)
            .map_err(|error| map_attribute_set_error(target, error))?;
    }
    Ok(())
}

fn stack_multiplier(policy: StackMagnitudePolicy, stack_count: u32) -> u32 {
    match policy {
        StackMagnitudePolicy::None => 1,
        StackMagnitudePolicy::Linear => stack_count,
    }
}
