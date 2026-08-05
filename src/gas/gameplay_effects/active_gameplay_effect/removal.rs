use super::super::EffectSystemParams;
use super::planning::GameplayEffectApplicationError;
use super::requirements::{
    resolve_active_effect_tag_requirements, resolve_active_effect_tag_requirements_if_dirty,
};
use super::state::{ActiveEffectHandle, ActiveGameplayEffect, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagError, GameplayTagManager,
    tag_bits_from_tags_with_manager,
};
use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(super) struct EffectCleanupResources<'a, 'w> {
    pub(super) attribute_id_manager: &'a AttributeIdManager,
    pub(super) tag_manager: &'a Res<'w, GameplayTagManager>,
}

/// Removes an active effect and cleans up its modifiers and granted tags.
///
/// Returns `Ok(false)` for a stale or missing handle.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if cleanup fails.
pub fn remove_active_effect(
    handle: ActiveEffectHandle,
    params: &mut EffectSystemParams,
) -> Result<bool, GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let effect = {
        let Ok(active_effects) = params.active_effect_query.get(handle.get_target()) else {
            return Ok(false);
        };
        let Some(effect) = active_effects.get(handle) else {
            return Ok(false);
        };
        effect.clone()
    };
    cleanup_effect_state(
        handle,
        &effect,
        EffectCleanupResources {
            attribute_id_manager: &params.attribute_id_manager,
            tag_manager: &params.tag_manager,
        },
        &mut params.attr_set_query,
        &mut params.tag_container_query,
    )?;
    if let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) {
        active_effects.remove(handle);
    }
    params.active_effect_requirement_sync.mark_dirty();
    resolve_active_effect_tag_requirements_if_dirty(params);
    Ok(true)
}

/// Removes active effects on `target` whose asset tags match `tags`.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if matching or cleanup fails.
pub fn remove_active_effects_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    params: &mut EffectSystemParams,
) -> Result<usize, GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let handles = collect_active_effects_with_tags_for_params(
        target,
        tags,
        &params.active_effect_query,
        &params.tag_manager,
    )?;
    let removed_count = handles.len();
    remove_collected_active_effects_for_params(&handles, params)?;
    if removed_count > 0 {
        params.active_effect_requirement_sync.mark_dirty();
        resolve_active_effect_tag_requirements_if_dirty(params);
    }
    Ok(removed_count)
}

/// Returns active handles in stable slot order.
pub fn get_active_effects_on_target(
    target: Entity,
    active_effects: &ActiveGameplayEffects,
) -> Vec<ActiveEffectHandle> {
    active_effects.handles(target).collect()
}

/// Tests whether a container has an active effect matching any supplied tag.
///
/// # Errors
///
/// Returns [`GameplayTagError`] when a queried tag is invalid.
pub fn has_active_effect_with_tags(
    active_effects: &ActiveGameplayEffects,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> Result<bool, GameplayTagError> {
    if tags.is_empty() {
        return Ok(false);
    }
    for handle in active_effects.stored_handles() {
        let Some(effect) = active_effects.get(handle) else {
            continue;
        };
        if active_effect_has_any_tags(effect, tags, tag_manager)? {
            return Ok(true);
        }
    }
    Ok(false)
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
    if let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) {
        attributes.remove_modifiers_for_attributes(
            resources.attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        )?;
    }
    if let Ok(mut tags) = tag_query.get_mut(effect.get_target()) {
        tags.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            resources.tag_manager,
        )?;
    }
    Ok(())
}

pub(super) fn force_remove_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    active_effects: &mut ActiveGameplayEffects,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
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

pub(super) fn collect_active_effects_with_tags_for_params(
    target: Entity,
    tags: &[GameplayTag],
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<Vec<ActiveEffectHandle>, GameplayTagError> {
    if tags.is_empty() {
        return Ok(Vec::new());
    }
    let Ok(active_effects) = active_effect_query.get(target) else {
        return Ok(Vec::new());
    };
    let mut matches = Vec::new();
    for handle in active_effects.handles(target) {
        let Some(effect) = active_effects.get(handle) else {
            continue;
        };
        if active_effect_has_any_tags(effect, tags, tag_manager)? {
            matches.push(handle);
        }
    }
    Ok(matches)
}

pub(super) fn remove_collected_active_effects_for_params(
    handles: &[ActiveEffectHandle],
    params: &mut EffectSystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let effects: Vec<_> = handles
        .iter()
        .filter_map(|&handle| {
            params
                .active_effect_query
                .get(handle.get_target())
                .ok()
                .and_then(|active_effects| active_effects.get(handle).cloned())
                .map(|effect| (handle, effect))
        })
        .collect();
    for (_, effect) in &effects {
        validate_effect_cleanup(
            effect,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
        )?;
    }
    for (handle, effect) in effects {
        cleanup_effect_state(
            handle,
            &effect,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
            &mut params.attr_set_query,
            &mut params.tag_container_query,
        )?;
        if let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) {
            active_effects.remove(handle);
        }
    }
    Ok(())
}

fn active_effect_has_any_tags(
    effect: &ActiveGameplayEffect,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> Result<bool, GameplayTagError> {
    let effect_bits = tag_bits_from_tags_with_manager(
        effect.get_spec().get_def_tags().get_asset_tags(),
        tag_manager,
    )?;
    let query_bits = tag_bits_from_tags_with_manager(tags, tag_manager)?;
    Ok(effect_bits
        .iter()
        .zip(query_bits.iter())
        .any(|(effect_bits, query_bits)| (effect_bits & query_bits) != 0))
}
