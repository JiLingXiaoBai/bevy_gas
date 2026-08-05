use crate::gameplay_tags::{
    GameplayTag, GameplayTagBits, GameplayTagContainer, GameplayTagError, GameplayTagManager,
};
use bevy::prelude::Res;

pub use crate::gameplay_tags::TagRequirements;

#[derive(Default)]
pub struct GameplayEffectImmunityQuery {
    source_tags: TagRequirements,
    effect_tags: TagRequirements,
}

impl GameplayEffectImmunityQuery {
    pub fn new(source_tags: TagRequirements, effect_tags: TagRequirements) -> Self {
        Self {
            source_tags,
            effect_tags,
        }
    }

    pub fn matches(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_tags: &[GameplayTag],
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError> {
        Ok(self.source_tags.passes(source_tags)
            && self
                .effect_tags
                .passes_tag_slice(effect_asset_tags, tag_manager)?)
    }

    pub fn matches_tag_bits(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_bits: Option<&GameplayTagBits>,
    ) -> bool {
        self.source_tags.passes(source_tags)
            && effect_asset_bits.is_some_and(|bits| self.effect_tags.passes_tag_bits(bits))
    }
}

pub struct EffectTags {
    asset_tags: Vec<GameplayTag>,
    granted_tags: Vec<GameplayTag>,
    source_application_tags: TagRequirements,
    target_application_tags: TagRequirements,
    source_ongoing_tags: TagRequirements,
    target_ongoing_tags: TagRequirements,
    source_removal_tags: TagRequirements,
    target_removal_tags: TagRequirements,
    granted_application_immunity: Vec<GameplayEffectImmunityQuery>,
    remove_effects_with_tags: Vec<GameplayTag>,
}

impl EffectTags {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset_tags: Vec<GameplayTag>,
        granted_tags: Vec<GameplayTag>,
        source_application_tags: TagRequirements,
        target_application_tags: TagRequirements,
        source_ongoing_tags: TagRequirements,
        target_ongoing_tags: TagRequirements,
        source_removal_tags: TagRequirements,
        target_removal_tags: TagRequirements,
        granted_application_immunity: Vec<GameplayEffectImmunityQuery>,
        remove_effects_with_tags: Vec<GameplayTag>,
    ) -> Self {
        Self {
            asset_tags,
            granted_tags,
            source_application_tags,
            target_application_tags,
            source_ongoing_tags,
            target_ongoing_tags,
            source_removal_tags,
            target_removal_tags,
            granted_application_immunity,
            remove_effects_with_tags,
        }
    }

    pub fn get_asset_tags(&self) -> &[GameplayTag] {
        &self.asset_tags
    }

    pub fn get_granted_tags(&self) -> &[GameplayTag] {
        &self.granted_tags
    }

    pub fn get_required_tags(&self) -> &[GameplayTag] {
        self.target_application_tags.get_required_tags()
    }

    pub fn get_blocked_tags(&self) -> &[GameplayTag] {
        self.target_application_tags.get_ignored_tags()
    }

    pub fn get_source_application_tags(&self) -> &TagRequirements {
        &self.source_application_tags
    }

    pub fn get_target_application_tags(&self) -> &TagRequirements {
        &self.target_application_tags
    }

    pub fn get_source_ongoing_tags(&self) -> &TagRequirements {
        &self.source_ongoing_tags
    }

    pub fn get_target_ongoing_tags(&self) -> &TagRequirements {
        &self.target_ongoing_tags
    }

    pub fn get_source_removal_tags(&self) -> &TagRequirements {
        &self.source_removal_tags
    }

    pub fn get_target_removal_tags(&self) -> &TagRequirements {
        &self.target_removal_tags
    }

    pub fn get_granted_application_immunity(&self) -> &[GameplayEffectImmunityQuery] {
        &self.granted_application_immunity
    }

    pub fn get_remove_effects_with_tags(&self) -> &[GameplayTag] {
        &self.remove_effects_with_tags
    }
}
