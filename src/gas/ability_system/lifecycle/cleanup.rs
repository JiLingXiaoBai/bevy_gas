use super::{
    AbilityActivationStatus, AbilitySystemComponent, ActiveGameplayAbility, GameplayTagManager,
    PendingActiveGameplayAbilities,
};
use bevy::ecs::lifecycle::Discard;
use bevy::prelude::*;

/// Releases ended or cancelled activations and despawns their task hierarchy.
/// Pending instances have already been applied at the default Cleanup schedule boundary.
pub fn cleanup_finished_abilities_system(
    mut commands: Commands,
    active_query: Query<(Entity, &ActiveGameplayAbility)>,
    mut asc_query: Query<&mut AbilitySystemComponent>,
    tag_manager: Res<GameplayTagManager>,
    mut pending: ResMut<PendingActiveGameplayAbilities>,
) {
    pending.clear();
    let mut ending: Vec<_> = active_query
        .iter()
        .filter(|(_, active)| {
            matches!(
                active.get_status(),
                AbilityActivationStatus::Ending | AbilityActivationStatus::Cancelled
            )
        })
        .map(|(entity, active)| (entity, active.get_source()))
        .collect();
    ending.sort_by_key(|(entity, _)| entity.to_bits());
    for (entity, source) in ending {
        if let Ok(mut asc) = asc_query.get_mut(source) {
            if let Err(error) = asc.finish_active_ability(entity, &mut commands, &tag_manager) {
                error!("failed to finish an active ability: {error}");
                asc.discard_active_ability(entity, &mut commands);
            }
        } else {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Component replacement and removal terminate the old activation, including direct ECS removal.
pub(crate) fn cleanup_discarded_active_ability(
    event: On<Discard, ActiveGameplayAbility>,
    active_query: Query<&ActiveGameplayAbility>,
    mut asc_query: Query<&mut AbilitySystemComponent>,
    tag_manager: Res<GameplayTagManager>,
    mut commands: Commands,
    mut pending: ResMut<PendingActiveGameplayAbilities>,
) {
    let Ok(active) = active_query.get(event.entity) else {
        return;
    };
    if let Ok(mut asc) = asc_query.get_mut(active.get_source())
        && let Err(error) = asc.release_active_ability(event.entity, &tag_manager)
    {
        error!("failed to release a discarded ability: {error}");
        asc.discard_active_ability(event.entity, &mut commands);
    }
    pending.remove(event.entity);
    commands.entity(event.entity).try_despawn();
}

/// Discarding an ASC terminates all of its registered activation entities.
pub(crate) fn cleanup_discarded_ability_system(
    event: On<Discard, AbilitySystemComponent>,
    asc_query: Query<&AbilitySystemComponent>,
    mut commands: Commands,
    mut pending: ResMut<PendingActiveGameplayAbilities>,
) {
    let Ok(asc) = asc_query.get(event.entity) else {
        return;
    };
    for &(handle, _) in &asc.active_instances {
        pending.remove(handle);
        commands.entity(handle).try_despawn();
    }
}
