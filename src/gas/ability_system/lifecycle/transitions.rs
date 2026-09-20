use super::super::commit::AdditionalCostProvider;
use super::{
    AbilityActivationStatus, AbilitySystemParams, ActiveAbilityHandle, ActiveGameplayAbility,
    GameplayAbility, GameplayTag, GameplayTagError, GameplayTagManager,
    tag_bits_from_tags_with_manager,
};
use bevy::prelude::*;

/// Marks an owned activation as ending; Cleanup releases its bookkeeping and task entities.
/// Returns whether a visible or pending activation belonging to `source` was found.
pub fn end_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Ending,
        params,
    )
}

/// Marks an owned activation as cancelled; Cleanup releases its bookkeeping and task entities.
/// Returns whether a visible or pending activation belonging to `source` was found.
pub fn cancel_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Cancelled,
        params,
    )
}

/// Updates visible and deferred instances without changing ASC bookkeeping or cleanup timing.
pub(in crate::gas::ability_system) fn finish_ability_with_status(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    status: AbilityActivationStatus,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) -> bool {
    let mut updated = false;
    if let Ok((_, mut active_ability)) = params.active_ability_query.get_mut(active_handle)
        && active_ability.get_source() == source
    {
        active_ability.set_status(status);
        updated = true;
    }

    let pending_update = params
        .pending_active_abilities
        .get_mut(active_handle)
        .filter(|active_ability| active_ability.get_source() == source)
        .map(|active_ability| {
            active_ability.set_status(status);
            active_ability.get_status()
        });
    if let Some(status) = pending_update {
        params.commands.queue(move |world: &mut World| {
            if let Some(mut active) = world.get_mut::<ActiveGameplayAbility>(active_handle)
                && active.get_source() == source
            {
                active.set_status(status);
            }
        });
        updated = true;
    }
    updated
}

pub(in crate::gas::ability_system) fn cancel_active_abilities_with_tags(
    source: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) -> Result<(), GameplayTagError> {
    if tags.is_empty() {
        return Ok(());
    }

    let mut active_instances: Vec<_> = params
        .active_ability_query
        .iter()
        .filter(|(_, active)| {
            active.get_source() == source
                && matches!(active.get_status(), AbilityActivationStatus::Active)
        })
        .map(|(active_handle, active)| (active_handle, active.get_spec_handle()))
        .collect();
    active_instances.extend(
        params
            .pending_active_abilities
            .iter()
            .filter(|(_, active)| {
                active.get_source() == source
                    && matches!(active.get_status(), AbilityActivationStatus::Active)
            })
            .map(|(active_handle, active)| (active_handle, active.get_spec_handle())),
    );
    active_instances.sort_by_key(|(active_handle, _)| active_handle.to_bits());
    active_instances.dedup_by_key(|(active_handle, _)| *active_handle);

    let active_instances = {
        let Ok(asc) = params.asc_query.get(source) else {
            return Ok(());
        };
        let mut matching_instances = Vec::new();
        for (active_handle, spec_handle) in active_instances {
            let Some(spec) = asc.find_ability_spec(spec_handle) else {
                continue;
            };
            if ability_has_any_tags(spec.get_ability(), tags, &params.effects.tag_manager)? {
                tag_bits_from_tags_with_manager(
                    spec.get_ability()
                        .get_tags()
                        .get_block_abilities_with_tags(),
                    &params.effects.tag_manager,
                )?;
                matching_instances.push(active_handle);
            }
        }
        matching_instances
    };

    for active_handle in active_instances {
        finish_ability_with_status(
            source,
            active_handle,
            AbilityActivationStatus::Cancelled,
            params,
        );
        if let Ok(mut asc) = params.asc_query.get_mut(source) {
            asc.finish_active_ability(
                active_handle,
                &mut params.commands,
                &params.effects.tag_manager,
            )?;
            params.pending_active_abilities.remove(active_handle);
        }
    }
    Ok(())
}

fn ability_has_any_tags(
    ability: &GameplayAbility,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> Result<bool, GameplayTagError> {
    let ability_bits =
        tag_bits_from_tags_with_manager(ability.get_tags().get_ability_asset_tags(), tag_manager)?;
    let query_bits = tag_bits_from_tags_with_manager(tags, tag_manager)?;

    Ok(ability_bits
        .iter()
        .zip(query_bits.iter())
        .any(|(a, b)| (a & b) != 0))
}
