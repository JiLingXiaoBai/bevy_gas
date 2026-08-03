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
    input_id: Option<u16>,
    input_pressed: bool,
    active_count: u32,
}

impl GameplayAbilitySpec {
    pub fn new(
        handle: AbilitySpecHandle,
        ability: Arc<GameplayAbility>,
        level: u32,
        input_id: Option<u16>,
    ) -> Self {
        Self {
            handle,
            ability,
            level,
            input_id,
            input_pressed: false,
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

    pub fn get_input_id(&self) -> Option<u16> {
        self.input_id
    }

    /// Returns whether the input bound to this ability is currently pressed.
    pub fn is_input_pressed(&self) -> bool {
        self.input_pressed
    }

    /// Updates whether the input bound to this ability is currently pressed.
    pub fn set_input_pressed(&mut self, input_pressed: bool) {
        self.input_pressed = input_pressed;
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
