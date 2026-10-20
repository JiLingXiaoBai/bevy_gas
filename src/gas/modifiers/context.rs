use crate::attributes::{
    AttributeId, AttributeIdError, AttributeIdManager, AttributeSetSnapshot, AttributeSnapshot,
};
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

    /// Returns the source's live base and current values for `id`.
    ///
    /// Live reads include current modifiers without changing cached values or dirty state.
    /// `Ok(None)` means live data is unavailable or the attribute is uninitialized. The default
    /// implementation validates `id` and returns `Ok(None)` for contexts without live access.
    /// This does not fall back to `source_snapshot`; callers choose that explicitly.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered in this context.
    fn source_attribute(
        &self,
        id: AttributeId,
    ) -> Result<Option<AttributeSnapshot>, AttributeIdError> {
        self.attribute_id_manager().location(id)?;
        Ok(None)
    }

    /// Returns the target's live base and current values for `id`.
    ///
    /// Live reads include current modifiers without changing cached values or dirty state.
    /// `Ok(None)` means the target, its attributes, or this initialized attribute is unavailable.
    /// The default implementation validates `id` and returns `Ok(None)` for contexts without
    /// live access.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered in this context.
    fn target_attribute(
        &self,
        id: AttributeId,
    ) -> Result<Option<AttributeSnapshot>, AttributeIdError> {
        self.attribute_id_manager().location(id)?;
        Ok(None)
    }

    /// Returns the registry used to resolve attribute storage locations.
    fn attribute_id_manager(&self) -> &AttributeIdManager;

    /// Returns the source's live gameplay tags, when available.
    fn source_tags(&self) -> Option<&GameplayTagContainer>;

    /// Returns the target's live gameplay tags, when available.
    fn target_tags(&self) -> Option<&GameplayTagContainer>;
}
