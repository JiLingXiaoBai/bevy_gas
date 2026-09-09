//! Shared modifier mutation for active-effect application and lifecycle changes.

use super::super::gameplay_effect::StackMagnitudePolicy;
use super::super::gameplay_effect_spec::GameplayEffectSpec;
use super::planning::{GameplayEffectApplicationError, map_attribute_set_error};
use super::state::ActiveEffectHandle;
use crate::attributes::{AttributeIdManager, AttributeSet};
use bevy::prelude::Entity;

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

/// Replaces this effect's retained modifiers using its current stack count.
///
/// Removal precedes reapplication; callers retain responsibility for failure cleanup.
pub(super) fn refresh_duration_modifiers(
    target: Entity,
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    handle: ActiveEffectHandle,
    stack_count: u32,
) -> Result<(), GameplayEffectApplicationError> {
    attr_set.remove_modifiers_for_attributes(
        attribute_id_manager,
        handle,
        spec.get_modified_attribute_ids(),
    )?;
    apply_duration_modifiers(
        target,
        attr_set,
        attribute_id_manager,
        spec,
        handle,
        stack_count,
    )
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
