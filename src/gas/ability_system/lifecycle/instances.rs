use super::{
    AbilityActivationRequest, AbilityActivationStatus, AbilitySystemComponent, ActiveAbilityHandle,
    ActiveGameplayAbility, GameplayTagError, GameplayTagManager, PendingActiveGameplayAbilities,
};
use bevy::prelude::*;

impl AbilitySystemComponent {
    pub(in crate::gas::ability_system) fn start_ability(
        &mut self,
        request: &AbilityActivationRequest,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
        pending: &mut PendingActiveGameplayAbilities,
    ) -> Result<ActiveAbilityHandle, GameplayTagError> {
        let blocked_tags = self
            .find_ability_spec(request.get_handle())
            .map(|spec| {
                spec.get_ability()
                    .get_tags()
                    .get_block_abilities_with_tags()
                    .to_vec()
            })
            .unwrap_or_default();
        self.blocked_ability_tags_mut()
            .add_tags(&blocked_tags, tag_manager)?;
        let active = ActiveGameplayAbility::from_data(
            request.get_handle(),
            request.get_activation_data().clone(),
            AbilityActivationStatus::Active,
        );
        let mut entity = commands.spawn(active.clone());
        let active_handle = entity.id();
        entity.set_parent_in_place(request.get_source());
        self.active_instances
            .push((active_handle, request.get_handle()));
        if let Some(spec) = self.find_ability_spec_mut(request.get_handle()) {
            spec.increment_active_count();
        }
        pending.insert(active_handle, active);
        Ok(active_handle)
    }

    /// Releases bookkeeping only while this ASC still owns the activation.
    pub(in crate::gas::ability_system) fn release_active_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        tag_manager: &GameplayTagManager,
    ) -> Result<bool, GameplayTagError> {
        let Some((_, spec_handle)) = self
            .active_instances
            .iter()
            .find(|(handle, _)| *handle == active_handle)
            .copied()
        else {
            return Ok(false);
        };
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
        self.forget_active_ability(active_handle);
        Ok(true)
    }

    pub(in crate::gas::ability_system) fn finish_active_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        commands: &mut Commands,
        tag_manager: &GameplayTagManager,
    ) -> Result<bool, GameplayTagError> {
        let released = self.release_active_ability(active_handle, tag_manager)?;
        commands.entity(active_handle).try_despawn();
        Ok(released)
    }

    pub(in crate::gas::ability_system) fn discard_active_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        commands: &mut Commands,
    ) {
        self.forget_active_ability(active_handle);
        commands.entity(active_handle).try_despawn();
    }

    fn forget_active_ability(&mut self, active_handle: ActiveAbilityHandle) {
        let Some(index) = self
            .active_instances
            .iter()
            .position(|(handle, _)| *handle == active_handle)
        else {
            return;
        };
        let (_, spec_handle) = self.active_instances.remove(index);
        if let Some(spec) = self.find_ability_spec_mut(spec_handle) {
            spec.decrement_active_count();
        }
    }
}

/// Validates ownership after all spawn and task commands for this activation have been applied.
/// A previously queued ASC replacement can run before these commands become visible.
pub(in crate::gas::ability_system) fn finish_ability_startup(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    commands: &mut Commands,
) {
    commands.queue(move |world: &mut World| {
        let owned = world
            .get::<AbilitySystemComponent>(source)
            .is_some_and(|asc| {
                asc.active_instances
                    .iter()
                    .any(|(handle, _)| *handle == active_handle)
            });
        if !owned && let Ok(entity) = world.get_entity_mut(active_handle) {
            entity.despawn();
        }
    });
}
