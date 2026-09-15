use super::{ActiveEffectRequirementSync, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{GameplayTagContainer, GameplayTagManager};
use crate::randoms::Random;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// ECS access required to prepare, apply, update, and remove gameplay effects.
///
/// This parameter intentionally excludes ability instances and the ability-system component so
/// effect APIs can be used independently from ability activation orchestration.
#[derive(SystemParam)]
pub struct EffectSystemParams<'w, 's> {
    /// Registered gameplay tags used to validate and build hierarchical tag bitsets.
    pub tag_manager: Res<'w, GameplayTagManager>,
    /// Deterministic random source used by probabilistic effect application.
    pub random_gen: ResMut<'w, Random>,
    /// Registered attribute IDs and their physical storage locations.
    pub attribute_id_manager: Res<'w, AttributeIdManager>,
    /// Mutable attribute storage for effect targets.
    pub attr_set_query: Query<'w, 's, &'static mut AttributeSet>,
    /// Mutable gameplay-tag storage for effect sources and targets.
    pub tag_container_query: Query<'w, 's, &'static mut GameplayTagContainer>,
    /// Active duration and infinite effects owned by target entities.
    pub active_effect_query: Query<'w, 's, &'static mut ActiveGameplayEffects>,
    pub(crate) active_effect_requirement_sync: ResMut<'w, ActiveEffectRequirementSync>,
}

/// Read-only ECS access for effect evaluation and attribute-cost previews.
///
/// This parameter neither borrows the random generator nor grants mutation access to gameplay state.
#[derive(SystemParam)]
pub struct EffectReadOnlyParams<'w, 's> {
    /// Registered gameplay tags used to inspect effect removal rules.
    pub tag_manager: Res<'w, GameplayTagManager>,
    /// Registered attribute IDs and their storage locations.
    pub attribute_id_manager: Res<'w, AttributeIdManager>,
    /// Attribute storage consulted by modifier evaluation and cost previews.
    pub attr_set_query: Query<'w, 's, &'static AttributeSet>,
    /// Gameplay tags consulted by modifier evaluation.
    pub tag_container_query: Query<'w, 's, &'static GameplayTagContainer>,
    /// Active effects whose modifiers may be excluded during a cost preview.
    pub active_effect_query: Query<'w, 's, &'static ActiveGameplayEffects>,
}
