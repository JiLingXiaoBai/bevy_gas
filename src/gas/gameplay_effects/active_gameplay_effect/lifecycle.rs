//! Shared installation and cleanup of effect-owned attribute and tag contributions.

use super::super::gameplay_effect_spec::GameplayEffectSpec;
use super::modifiers::apply_duration_modifiers;
use super::planning::{GameplayEffectApplicationError, map_attribute_set_error};
use super::requirements::ActiveEffectRequirementSync;
use super::state::{ActiveEffectHandle, ActiveGameplayEffect, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{
    GameplayTagContainer, GameplayTagManager, tag_bits_from_tags_with_manager,
};
use bevy::ecs::{lifecycle::HookContext, world::DeferredWorld};
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(super) struct EffectCleanupResources<'a> {
    pub(super) attribute_id_manager: &'a AttributeIdManager,
    pub(super) tag_manager: &'a GameplayTagManager,
}

/// Installs the retained contributions of a new or restored effect.
///
/// Tags are installed only after all retained attributes have been validated. If a
/// later modifier installation fails, this function removes the contributions it installed.
pub(super) fn install_effect_contributions(
    handle: ActiveEffectHandle,
    spec: &GameplayEffectSpec,
    stack_count: u32,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
) -> Result<(), GameplayEffectApplicationError> {
    let target = handle.get_target();
    let retains_modifiers = spec
        .get_period_spec()
        .as_ref()
        .is_none_or(|period| period.get_period_ticks() == 0)
        && !spec.get_modifier_specs().is_empty();
    let granted_tags = spec.get_def_tags().get_granted_tags();
    tag_bits_from_tags_with_manager(granted_tags, resources.tag_manager)?;
    if retains_modifiers {
        let attributes = attr_query
            .get(target)
            .map_err(|_| GameplayEffectApplicationError::MissingAttributeSet { target })?;
        for modifier in spec.get_modifier_specs() {
            attributes
                .validate_initialized_attribute(resources.attribute_id_manager, modifier.get_id())
                .map_err(|error| map_attribute_set_error(target, error))?;
        }
    }
    if !granted_tags.is_empty() {
        let mut tags = tag_query
            .get_mut(target)
            .map_err(|_| GameplayEffectApplicationError::MissingTagContainer { target })?;
        tags.add_tags(granted_tags, resources.tag_manager)?;
    }
    if retains_modifiers {
        let result = match attr_query.get_mut(target) {
            Ok(mut attributes) => apply_duration_modifiers(
                target,
                &mut attributes,
                resources.attribute_id_manager,
                spec,
                handle,
                stack_count,
            ),
            Err(_) => Err(GameplayEffectApplicationError::MissingAttributeSet { target }),
        };
        if let Err(error) = result {
            remove_effect_contributions(handle, spec, resources, attr_query, tag_query)?;
            return Err(error);
        }
    }
    Ok(())
}

/// Removes contributions regardless of the effect's inhibited state.
pub(super) fn remove_effect_contributions(
    handle: ActiveEffectHandle,
    spec: &GameplayEffectSpec,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(mut attributes) = attr_query.get_mut(handle.get_target()) {
        attributes.remove_modifiers_for_attributes(
            resources.attribute_id_manager,
            handle,
            spec.get_modified_attribute_ids(),
        )?;
    }
    if let Ok(mut tags) = tag_query.get_mut(handle.get_target()) {
        tags.remove_tags(
            spec.get_def_tags().get_granted_tags(),
            resources.tag_manager,
        )?;
    }
    Ok(())
}

pub(super) fn validate_effect_cleanup(
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
) -> Result<(), GameplayEffectApplicationError> {
    for id in effect.get_spec().get_modified_attribute_ids() {
        resources.attribute_id_manager.location(id)?;
    }
    tag_bits_from_tags_with_manager(
        effect.get_spec().get_def_tags().get_granted_tags(),
        resources.tag_manager,
    )?;
    Ok(())
}

pub(super) fn cleanup_effect_state(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
) -> Result<(), GameplayEffectApplicationError> {
    validate_effect_cleanup(effect, resources)?;
    if effect.is_inhibited() {
        return Ok(());
    }
    remove_effect_contributions(handle, effect.get_spec(), resources, attr_query, tag_query)
}

pub(super) fn force_remove_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    active_effects: &mut ActiveGameplayEffects,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &GameplayTagManager,
) {
    if let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) {
        attributes.remove_modifiers(handle);
    }
    if !effect.is_inhibited()
        && let Ok(mut tags) = tag_query.get_mut(effect.get_target())
        && let Err(error) = tags.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )
    {
        error!("failed to force-remove gameplay effect tags: {error}");
    }
    active_effects.remove(handle);
}

/// Allocates deterministic identities for effect-container installations.
#[derive(Resource)]
pub(crate) struct ActiveEffectStorageRegistry {
    next_id: Option<u64>,
}

impl Default for ActiveEffectStorageRegistry {
    fn default() -> Self {
        Self { next_id: Some(1) }
    }
}

pub(super) fn initialize_effect_container(mut world: DeferredWorld, context: HookContext) {
    let storage_id = world
        .get_resource_mut::<ActiveEffectStorageRegistry>()
        .and_then(|mut registry| {
            let id = registry.next_id?;
            registry.next_id = id.checked_add(1);
            Some(id)
        })
        .unwrap_or(0);
    if let Some(mut effects) = world.get_mut::<ActiveGameplayEffects>(context.entity) {
        effects.initialize_storage(storage_id);
    }
}

pub(super) fn discard_effect_container(mut world: DeferredWorld, context: HookContext) {
    let effects = world
        .get_mut::<ActiveGameplayEffects>(context.entity)
        .map(|mut effects| effects.take_effects())
        .unwrap_or_default();
    if effects.is_empty() {
        return;
    }
    if let Some(mut attributes) = world.get_mut::<AttributeSet>(context.entity) {
        for (handle, _) in &effects {
            attributes.remove_modifiers(*handle);
        }
    }
    if effects.iter().any(|(_, effect)| {
        !effect.is_inhibited()
            && !effect
                .get_spec()
                .get_def_tags()
                .get_granted_tags()
                .is_empty()
    }) {
        // Temporarily own the old tag value to borrow the registry safely. No callbacks
        // run before it is restored. Deferred cleanup would instead see replacement tags.
        let old_tags = world
            .get_mut::<GameplayTagContainer>(context.entity)
            .map(|mut tags| std::mem::take(&mut *tags));
        if let Some(mut tags) = old_tags {
            if let Some(manager) = world.get_resource::<GameplayTagManager>() {
                for (_, effect) in &effects {
                    if !effect.is_inhibited()
                        && let Err(error) = tags.remove_tags(
                            effect.get_spec().get_def_tags().get_granted_tags(),
                            manager,
                        )
                    {
                        error!("failed to clean up discarded gameplay effect tags: {error}");
                    }
                }
            } else {
                error!("cannot clean up discarded gameplay effect tags without GameplayTagManager");
            }
            if let Some(mut destination) = world.get_mut::<GameplayTagContainer>(context.entity) {
                *destination = tags;
            }
        }
    }
    if let Some(mut sync) = world.get_resource_mut::<ActiveEffectRequirementSync>() {
        sync.mark_dirty();
    }
}
