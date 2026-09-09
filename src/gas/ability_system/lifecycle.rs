use super::component::AbilitySystemComponent;
use super::params::{AbilitySystemParams, PendingActiveGameplayAbilities};
use crate::gameplay_abilities::{
    AbilityActivationStatus, AbilitySpecHandle, ActiveAbilityHandle, ActiveGameplayAbility,
    GameplayAbility,
};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagError, GameplayTagManager, tag_bits_from_tags_with_manager,
};
use bevy::prelude::*;

impl AbilitySystemComponent {
    fn finish_active_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        active_ability: &ActiveGameplayAbility,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError> {
        self.rollback_started_ability(
            active_handle,
            active_ability.get_spec_handle(),
            commands,
            tag_manager,
        )
    }

    pub(super) fn rollback_started_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        spec_handle: AbilitySpecHandle,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError> {
        let blocked_tags = self
            .find_ability_spec(spec_handle)
            .map(|spec| {
                spec.get_ability()
                    .get_tags()
                    .get_block_abilities_with_tags()
                    .to_vec()
            })
            .unwrap_or_default();

        self.blocked_ability_tags_mut()
            .remove_tags(&blocked_tags, tag_manager)?;
        if let Some(spec) = self.find_ability_spec_mut(spec_handle) {
            spec.decrement_active_count();
        }
        commands.entity(active_handle).despawn_children().despawn();
        Ok(true)
    }

    pub(super) fn discard_started_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        spec_handle: AbilitySpecHandle,
        commands: &mut Commands,
    ) {
        if let Some(spec) = self.find_ability_spec_mut(spec_handle) {
            spec.decrement_active_count();
        }
        commands.entity(active_handle).despawn_children().despawn();
    }
}

pub fn end_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Ending,
        params,
    )
}

pub fn cancel_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Cancelled,
        params,
    )
}

/// Updates visible and deferred instances without changing ASC bookkeeping or cleanup timing.
pub(super) fn finish_ability_with_status(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    status: AbilityActivationStatus,
    params: &mut AbilitySystemParams,
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
            active_ability.clone()
        });
    if let Some(active_ability) = pending_update {
        params.commands.entity(active_handle).insert(active_ability);
        updated = true;
    }
    updated
}

pub fn cleanup_finished_abilities_system(
    mut commands: Commands,
    active_ability_query: Query<(Entity, &ActiveGameplayAbility)>,
    mut asc_query: Query<&mut AbilitySystemComponent>,
    tag_manager: Res<GameplayTagManager>,
    mut pending_active_abilities: ResMut<PendingActiveGameplayAbilities>,
) {
    pending_active_abilities.clear();
    for (active_handle, active_ability) in active_ability_query.iter() {
        if !matches!(
            active_ability.get_status(),
            AbilityActivationStatus::Ending | AbilityActivationStatus::Cancelled
        ) {
            continue;
        }

        if let Ok(mut asc) = asc_query.get_mut(active_ability.get_source()) {
            if let Err(error) = asc.finish_active_ability(
                active_handle,
                active_ability,
                &mut commands,
                &tag_manager,
            ) {
                error!("failed to finish an active ability: {error}");
                asc.discard_started_ability(
                    active_handle,
                    active_ability.get_spec_handle(),
                    &mut commands,
                );
            }
        } else {
            commands.entity(active_handle).despawn_children().despawn();
        }
    }
}

pub(super) fn cancel_active_abilities_with_tags(
    source: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams,
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
        .map(|(active_handle, active)| (active_handle, active.clone()))
        .collect();
    active_instances.extend(
        params
            .pending_active_abilities
            .iter()
            .filter(|(_, active)| {
                active.get_source() == source
                    && matches!(active.get_status(), AbilityActivationStatus::Active)
            })
            .map(|(active_handle, active)| (active_handle, active.clone())),
    );
    active_instances.sort_by_key(|(active_handle, _)| active_handle.to_bits());
    active_instances.dedup_by_key(|(active_handle, _)| *active_handle);

    let active_instances = {
        let Ok(asc) = params.asc_query.get(source) else {
            return Ok(());
        };
        let mut matching_instances = Vec::new();
        for (active_handle, active) in active_instances {
            let Some(spec) = asc.find_ability_spec(active.get_spec_handle()) else {
                continue;
            };
            if ability_has_any_tags(spec.get_ability(), tags, &params.effects.tag_manager)? {
                tag_bits_from_tags_with_manager(
                    spec.get_ability()
                        .get_tags()
                        .get_block_abilities_with_tags(),
                    &params.effects.tag_manager,
                )?;
                matching_instances.push((active_handle, active));
            }
        }
        matching_instances
    };

    for (active_handle, active_ability) in active_instances {
        finish_ability_with_status(
            source,
            active_handle,
            AbilityActivationStatus::Cancelled,
            params,
        );
        if let Ok(mut asc) = params.asc_query.get_mut(source) {
            asc.finish_active_ability(
                active_handle,
                &active_ability,
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
