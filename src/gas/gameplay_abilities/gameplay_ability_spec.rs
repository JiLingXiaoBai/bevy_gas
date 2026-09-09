use super::GameplayAbility;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilitySpecHandle(u32);

impl AbilitySpecHandle {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn get_value(&self) -> u32 {
        self.0
    }
}

pub struct GameplayAbilitySpec {
    handle: AbilitySpecHandle,
    ability: Arc<GameplayAbility>,
    level: u32,
    active_count: u32,
}

impl GameplayAbilitySpec {
    /// Creates an inactive granted ability from its owner-local `handle`, shared `ability`
    /// definition, and granted `level`.
    pub fn new(handle: AbilitySpecHandle, ability: Arc<GameplayAbility>, level: u32) -> Self {
        Self {
            handle,
            ability,
            level,
            active_count: 0,
        }
    }

    pub fn get_handle(&self) -> AbilitySpecHandle {
        self.handle
    }

    pub fn get_ability(&self) -> &Arc<GameplayAbility> {
        &self.ability
    }

    pub fn get_level(&self) -> u32 {
        self.level
    }

    pub fn get_active_count(&self) -> u32 {
        self.active_count
    }

    pub fn increment_active_count(&mut self) {
        self.active_count = self.active_count.saturating_add(1);
    }

    pub fn decrement_active_count(&mut self) {
        self.active_count = self.active_count.saturating_sub(1);
    }
}
