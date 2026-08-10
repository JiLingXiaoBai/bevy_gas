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

/// Stores one lifecycle phase's source and target requirements together.
#[derive(Default)]
struct SourceTargetTagRequirements {
    source: TagRequirements,
    target: TagRequirements,
}

/// Configures an effect's identity, granted tags, requirements, immunity, and removal rules.
pub struct EffectTags {
    asset_tags: Vec<GameplayTag>,
    granted_tags: Vec<GameplayTag>,
    application_requirements: SourceTargetTagRequirements,
    ongoing_requirements: SourceTargetTagRequirements,
    removal_requirements: SourceTargetTagRequirements,
    granted_application_immunity: Vec<GameplayEffectImmunityQuery>,
    remove_effects_with_tags: Vec<GameplayTag>,
}

impl EffectTags {
    /// Creates effect tags without optional application, ongoing, removal, or immunity rules.
    pub fn new(asset_tags: Vec<GameplayTag>, granted_tags: Vec<GameplayTag>) -> Self {
        Self {
            asset_tags,
            granted_tags,
            application_requirements: SourceTargetTagRequirements::default(),
            ongoing_requirements: SourceTargetTagRequirements::default(),
            removal_requirements: SourceTargetTagRequirements::default(),
            granted_application_immunity: Vec::new(),
            remove_effects_with_tags: Vec::new(),
        }
    }

    /// Configures the source and target requirements checked before application.
    pub fn with_application_requirements(
        mut self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self {
        self.application_requirements = SourceTargetTagRequirements { source, target };
        self
    }

    /// Configures the source and target requirements checked while the effect is active.
    pub fn with_ongoing_requirements(
        mut self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self {
        self.ongoing_requirements = SourceTargetTagRequirements { source, target };
        self
    }

    /// Configures the source and target requirements that trigger effect removal.
    pub fn with_removal_requirements(
        mut self,
        source: TagRequirements,
        target: TagRequirements,
    ) -> Self {
        self.removal_requirements = SourceTargetTagRequirements { source, target };
        self
    }

    /// Configures immunity queries granted while this effect is active.
    pub fn with_granted_application_immunity(
        mut self,
        immunity_queries: Vec<GameplayEffectImmunityQuery>,
    ) -> Self {
        self.granted_application_immunity = immunity_queries;
        self
    }

    /// Configures asset tags used to remove matching active effects on application.
    pub fn with_remove_effects_with_tags(mut self, effect_tags: Vec<GameplayTag>) -> Self {
        self.remove_effects_with_tags = effect_tags;
        self
    }

    pub fn get_asset_tags(&self) -> &[GameplayTag] {
        &self.asset_tags
    }

    pub fn get_granted_tags(&self) -> &[GameplayTag] {
        &self.granted_tags
    }

    pub fn get_required_tags(&self) -> &[GameplayTag] {
        self.application_requirements.target.get_required_tags()
    }

    pub fn get_blocked_tags(&self) -> &[GameplayTag] {
        self.application_requirements.target.get_ignored_tags()
    }

    pub fn get_source_application_tags(&self) -> &TagRequirements {
        &self.application_requirements.source
    }

    pub fn get_target_application_tags(&self) -> &TagRequirements {
        &self.application_requirements.target
    }

    pub fn get_source_ongoing_tags(&self) -> &TagRequirements {
        &self.ongoing_requirements.source
    }

    pub fn get_target_ongoing_tags(&self) -> &TagRequirements {
        &self.ongoing_requirements.target
    }

    pub fn get_source_removal_tags(&self) -> &TagRequirements {
        &self.removal_requirements.source
    }

    pub fn get_target_removal_tags(&self) -> &TagRequirements {
        &self.removal_requirements.target
    }

    pub fn get_granted_application_immunity(&self) -> &[GameplayEffectImmunityQuery] {
        &self.granted_application_immunity
    }

    pub fn get_remove_effects_with_tags(&self) -> &[GameplayTag] {
        &self.remove_effects_with_tags
    }
}
