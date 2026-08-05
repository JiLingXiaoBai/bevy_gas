use super::component::AbilitySystemComponent;
use crate::attributes::AttributeSetSnapshot;
use crate::gameplay_abilities::{ActiveAbilityHandle, ActiveGameplayAbility};
use crate::gameplay_effects::EffectSystemParams;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::ops::{Deref, DerefMut};

#[derive(Resource, Default)]
#[doc(hidden)]
// This resource remains public because it appears in a public system signature. It is runtime
// plumbing, not a user-facing API contract; changing its visibility requires a separate API change.
pub struct PendingActiveGameplayAbilities {
    entries: Vec<(ActiveAbilityHandle, ActiveGameplayAbility)>,
}

impl PendingActiveGameplayAbilities {
    pub(super) fn insert(&mut self, handle: ActiveAbilityHandle, ability: ActiveGameplayAbility) {
        self.entries.push((handle, ability));
    }

    pub(super) fn get_mut(
        &mut self,
        handle: ActiveAbilityHandle,
    ) -> Option<&mut ActiveGameplayAbility> {
        self.entries
            .iter_mut()
            .find_map(|(entry_handle, ability)| (*entry_handle == handle).then_some(ability))
    }

    pub(super) fn iter(
        &self,
    ) -> impl Iterator<Item = (ActiveAbilityHandle, &ActiveGameplayAbility)> {
        self.entries
            .iter()
            .map(|(handle, ability)| (*handle, ability))
    }

    pub(super) fn remove(&mut self, handle: ActiveAbilityHandle) {
        self.entries
            .retain(|(entry_handle, _)| *entry_handle != handle);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn retain_unapplied(
        &mut self,
        active_ability_query: &Query<(Entity, &mut ActiveGameplayAbility)>,
    ) {
        self.entries
            .retain(|(handle, _)| active_ability_query.get(*handle).is_err());
    }
}

#[derive(SystemParam)]
pub struct AbilitySystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    /// Effect-specific ECS access shared with effect preparation and execution APIs.
    pub effects: EffectSystemParams<'w, 's>,
    pub asc_query: Query<'w, 's, &'static mut AbilitySystemComponent>,
    pub attr_set_snapshot_query: Query<'w, 's, &'static AttributeSetSnapshot>,
    pub active_ability_query: Query<'w, 's, (Entity, &'static mut ActiveGameplayAbility)>,
    pub(crate) pending_active_abilities: ResMut<'w, PendingActiveGameplayAbilities>,
}

impl<'w, 's> Deref for AbilitySystemParams<'w, 's> {
    type Target = EffectSystemParams<'w, 's>;

    fn deref(&self) -> &Self::Target {
        &self.effects
    }
}

impl<'w, 's> DerefMut for AbilitySystemParams<'w, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.effects
    }
}
