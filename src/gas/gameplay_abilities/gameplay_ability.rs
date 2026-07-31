use super::AbilityTaskDef;
use crate::gameplay_effects::GameplayEffect;
use crate::gameplay_tags::GameplayTag;
use std::sync::Arc;

#[derive(Default)]
pub struct AbilityTags {
    ability_asset_tags: Vec<GameplayTag>,
    cancel_abilities_with_tags: Vec<GameplayTag>,
    block_abilities_with_tags: Vec<GameplayTag>,
    activation_required_tags: Vec<GameplayTag>,
    activation_blocked_tags: Vec<GameplayTag>,
}

impl AbilityTags {
    pub fn new(
        ability_asset_tags: Vec<GameplayTag>,
        cancel_abilities_with_tags: Vec<GameplayTag>,
        block_abilities_with_tags: Vec<GameplayTag>,
        activation_required_tags: Vec<GameplayTag>,
        activation_blocked_tags: Vec<GameplayTag>,
    ) -> Self {
        Self {
            ability_asset_tags,
            cancel_abilities_with_tags,
            block_abilities_with_tags,
            activation_required_tags,
            activation_blocked_tags,
        }
    }

    pub fn get_ability_asset_tags(&self) -> &[GameplayTag] {
        &self.ability_asset_tags
    }

    pub fn get_cancel_abilities_with_tags(&self) -> &[GameplayTag] {
        &self.cancel_abilities_with_tags
    }

    pub fn get_block_abilities_with_tags(&self) -> &[GameplayTag] {
        &self.block_abilities_with_tags
    }

    pub fn get_activation_required_tags(&self) -> &[GameplayTag] {
        &self.activation_required_tags
    }

    pub fn get_activation_blocked_tags(&self) -> &[GameplayTag] {
        &self.activation_blocked_tags
    }
}

pub struct GameplayAbility {
    ability_tags: AbilityTags,
    startup_tasks: Vec<AbilityTaskDef>,
    cooldown: Option<Arc<GameplayEffect>>,
    cost: Option<Arc<GameplayEffect>>,
    activation_effects: Vec<Arc<GameplayEffect>>,
    end_on_activation: bool,
    allow_multiple_instances: bool,
}

impl GameplayAbility {
    pub fn new(
        ability_tags: AbilityTags,
        startup_tasks: Vec<AbilityTaskDef>,
        cooldown: Option<Arc<GameplayEffect>>,
        cost: Option<Arc<GameplayEffect>>,
        activation_effects: Vec<Arc<GameplayEffect>>,
        end_on_activation: bool,
        allow_multiple_instances: bool,
    ) -> Self {
        Self {
            ability_tags,
            startup_tasks,
            cooldown,
            cost,
            activation_effects,
            end_on_activation,
            allow_multiple_instances,
        }
    }

    pub fn get_tags(&self) -> &AbilityTags {
        &self.ability_tags
    }

    pub fn get_startup_tasks(&self) -> &[AbilityTaskDef] {
        &self.startup_tasks
    }

    pub fn get_cooldown(&self) -> Option<&Arc<GameplayEffect>> {
        self.cooldown.as_ref()
    }

    pub fn get_cost(&self) -> Option<&Arc<GameplayEffect>> {
        self.cost.as_ref()
    }

    pub fn get_activation_effects(&self) -> &[Arc<GameplayEffect>] {
        &self.activation_effects
    }

    pub fn should_end_on_activation(&self) -> bool {
        self.end_on_activation
    }

    pub fn allow_multiple_instances(&self) -> bool {
        self.allow_multiple_instances
    }
}
