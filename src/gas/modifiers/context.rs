use crate::attributes::{AttributeIdManager, AttributeSetSnapshot};
use crate::gameplay_tags::GameplayTagContainer;
use bevy::prelude::Entity;

/// Read-only gameplay data exposed to modifier magnitude calculations.
///
/// The interface is intentionally independent from gameplay-effect runtime
/// storage. Effect, ability, or other systems may implement it without making
/// modifier definitions depend on their concrete context types.
pub trait ModifierEvaluationContext {
    /// Returns the entity receiving the modifier, when one is known.
    fn target(&self) -> Option<Entity>;

    /// Returns the entity that provides the gameplay source state.
    fn source(&self) -> Entity;

    /// Returns the entity that initiated the action.
    fn instigator(&self) -> Entity;

    /// Returns the optional physical entity that directly caused the action.
    fn causer(&self) -> Option<Entity>;

    /// Returns the gameplay level used when evaluating scalable values.
    fn level(&self) -> u32;

    /// Returns the captured source attributes, when a snapshot was supplied.
    fn source_snapshot(&self) -> Option<&AttributeSetSnapshot>;

    /// Returns the registry used to resolve attribute storage locations.
    fn attribute_id_manager(&self) -> &AttributeIdManager;

    /// Returns the source's live gameplay tags, when available.
    fn source_tags(&self) -> Option<&GameplayTagContainer>;

    /// Returns the target's live gameplay tags, when available.
    fn target_tags(&self) -> Option<&GameplayTagContainer>;
}
