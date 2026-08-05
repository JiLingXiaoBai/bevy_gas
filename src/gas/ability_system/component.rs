use crate::attributes::AttributeSet;
use crate::gameplay_abilities::{AbilitySpecHandle, GameplayAbility, GameplayAbilitySpec};
use crate::gameplay_effects::ActiveGameplayEffects;
use crate::gameplay_tags::GameplayTagContainer;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::sync::Arc;

/// Stores the abilities granted to one gameplay entity.
#[derive(Component, Default)]
pub struct AbilitySystemComponent {
    next_ability_handle: u32,
    abilities: Vec<GameplayAbilitySpec>,
    ability_indices: HashMap<AbilitySpecHandle, usize>,
    blocked_ability_tags: GameplayTagContainer,
}

/// Explicitly installs the ECS components required by the complete gameplay ability system.
///
/// Individual tag and attribute components remain usable without active effects. Spawn this bundle
/// for entities that participate in the full ability, attribute, tag, and gameplay-effect runtime.
#[derive(Bundle, Default)]
pub struct GameplayAbilitySystemBundle {
    /// Granted abilities and per-ability runtime bookkeeping.
    pub ability_system: AbilitySystemComponent,
    /// Attribute values, aggregators, and dirty state.
    pub attributes: AttributeSet,
    /// Reference-counted owned gameplay tags.
    pub tags: GameplayTagContainer,
    /// Duration and infinite gameplay effects currently owned by the entity.
    pub active_effects: ActiveGameplayEffects,
}

impl AbilitySystemComponent {
    pub fn give_ability(
        &mut self,
        ability: Arc<GameplayAbility>,
        level: u32,
        input_id: Option<u16>,
    ) -> AbilitySpecHandle {
        let handle = AbilitySpecHandle::new(self.next_ability_handle);
        self.next_ability_handle = self.next_ability_handle.wrapping_add(1);
        let index = self.abilities.len();
        self.abilities
            .push(GameplayAbilitySpec::new(handle, ability, level, input_id));
        self.ability_indices.insert(handle, index);
        handle
    }

    pub fn clear_ability(&mut self, handle: AbilitySpecHandle) -> bool {
        if self
            .find_ability_spec(handle)
            .is_some_and(|spec| spec.get_active_count() > 0)
        {
            return false;
        }

        let old_len = self.abilities.len();
        self.abilities.retain(|spec| spec.get_handle() != handle);
        let removed = old_len != self.abilities.len();
        if removed {
            self.rebuild_ability_indices();
        }
        removed
    }

    pub fn get_ability_specs(&self) -> &[GameplayAbilitySpec] {
        &self.abilities
    }

    pub fn get_blocked_ability_tags(&self) -> &GameplayTagContainer {
        &self.blocked_ability_tags
    }

    pub fn find_ability_spec(&self, handle: AbilitySpecHandle) -> Option<&GameplayAbilitySpec> {
        self.ability_indices
            .get(&handle)
            .and_then(|&index| self.abilities.get(index))
    }

    pub(super) fn find_ability_spec_mut(
        &mut self,
        handle: AbilitySpecHandle,
    ) -> Option<&mut GameplayAbilitySpec> {
        self.ability_indices
            .get(&handle)
            .and_then(|&index| self.abilities.get_mut(index))
    }

    pub(super) fn blocked_ability_tags_mut(&mut self) -> &mut GameplayTagContainer {
        &mut self.blocked_ability_tags
    }

    fn rebuild_ability_indices(&mut self) {
        self.ability_indices.clear();
        for (index, spec) in self.abilities.iter().enumerate() {
            self.ability_indices.insert(spec.get_handle(), index);
        }
    }
}
