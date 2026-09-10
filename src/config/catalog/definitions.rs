use crate::{AttributeId, GameplayAbility, GameplayEffect, GameplayTag, TargetingDefinition};
use bevy::prelude::Resource;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Stable ability identifier authored in the configuration tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbilityId(pub i32);

/// Stable effect identifier authored in the configuration tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectId(pub i32);

/// An immutable configured ability with its separate targeting definition.
pub struct CompiledAbility {
    pub(crate) id: AbilityId,
    pub(crate) name: String,
    pub(crate) max_level: u32,
    pub(crate) definition: Arc<GameplayAbility>,
    pub(crate) targeting: Arc<TargetingDefinition>,
}

impl CompiledAbility {
    /// Returns the stable configuration ID.
    pub fn id(&self) -> AbilityId {
        self.id
    }
    /// Returns the authored display name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the maximum supported granted level.
    pub fn max_level(&self) -> u32 {
        self.max_level
    }
    /// Returns the shared GAS definition used when granting this ability.
    pub fn definition(&self) -> &Arc<GameplayAbility> {
        &self.definition
    }
    /// Returns the targeting pipeline to run before requesting activation.
    pub fn targeting(&self) -> &Arc<TargetingDefinition> {
        &self.targeting
    }
}

/// Startup-compiled definitions and name bindings shared by all gameplay actors.
#[derive(Resource)]
pub struct GameplayCatalog {
    pub(crate) abilities: BTreeMap<AbilityId, CompiledAbility>,
    pub(crate) effects: BTreeMap<EffectId, Arc<GameplayEffect>>,
    pub(crate) attributes: BTreeMap<String, AttributeId>,
    pub(crate) tags: BTreeMap<String, GameplayTag>,
}

impl GameplayCatalog {
    /// Returns a configured ability by stable `id`, or `None` when absent.
    pub fn ability(&self, id: AbilityId) -> Option<&CompiledAbility> {
        self.abilities.get(&id)
    }
    /// Returns the unique shared effect for `id`, or `None` when absent.
    pub fn effect(&self, id: EffectId) -> Option<&Arc<GameplayEffect>> {
        self.effects.get(&id)
    }
    /// Returns the runtime attribute ID for an authored `name`, when registered.
    pub fn attribute(&self, name: &str) -> Option<AttributeId> {
        self.attributes.get(name).copied()
    }
    /// Returns the runtime tag for an authored `name`, when registered.
    pub fn tag(&self, name: &str) -> Option<GameplayTag> {
        self.tags.get(name).copied()
    }
    /// Iterates configured abilities in ascending stable-ID order.
    pub fn abilities(&self) -> impl Iterator<Item = &CompiledAbility> {
        self.abilities.values()
    }
    /// Returns the number of shared effect definitions.
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }
}
