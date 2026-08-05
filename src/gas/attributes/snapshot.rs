use super::{
    AttributeId, AttributeIdError, AttributeIdManager, AttributeRegion, COLD_ATTRIBUTE_SET_SIZE,
    HOT_ATTRIBUTE_SET_SIZE,
};
use bevy::prelude::{Component, Entity};

/// Captured base and current values for one attribute.
#[derive(Debug, Clone, Copy)]
pub struct AttributeSnapshot {
    base: f32,
    current: f32,
}

impl AttributeSnapshot {
    /// Creates a captured attribute value.
    pub const fn new(base: f32, current: f32) -> Self {
        Self { base, current }
    }

    /// Returns the captured base value.
    pub const fn base(&self) -> f32 {
        self.base
    }

    /// Returns the captured current value.
    pub const fn current(&self) -> f32 {
        self.current
    }
}

/// Immutable snapshot of one entity's initialized attributes.
#[derive(Component, Clone)]
pub struct AttributeSetSnapshot {
    hot: [Option<AttributeSnapshot>; HOT_ATTRIBUTE_SET_SIZE],
    cold: Box<[Option<AttributeSnapshot>; COLD_ATTRIBUTE_SET_SIZE]>,
    source_entity: Entity,
}

impl AttributeSetSnapshot {
    pub(crate) fn new(
        hot: [Option<AttributeSnapshot>; HOT_ATTRIBUTE_SET_SIZE],
        cold: Box<[Option<AttributeSnapshot>; COLD_ATTRIBUTE_SET_SIZE]>,
        source_entity: Entity,
    ) -> Self {
        Self {
            hot,
            cold,
            source_entity,
        }
    }

    /// Returns the captured current value for `id`.
    ///
    /// `Ok(None)` means the ID is valid but was not initialized in the source.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered.
    pub fn get_current_value(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError> {
        self.get_attribute(manager, id)
            .map(|attribute| attribute.map(AttributeSnapshot::current))
    }

    /// Returns the captured base value for `id`.
    ///
    /// `Ok(None)` means the ID is valid but was not initialized in the source.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered.
    pub fn get_base_value(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError> {
        self.get_attribute(manager, id)
            .map(|attribute| attribute.map(AttributeSnapshot::base))
    }

    /// Returns the entity from which this snapshot was captured.
    pub const fn get_source_entity(&self) -> Entity {
        self.source_entity
    }

    fn get_attribute(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<&AttributeSnapshot>, AttributeIdError> {
        let location = manager.location(id)?;
        Ok(match location.region() {
            AttributeRegion::Hot => self.hot[location.slot()].as_ref(),
            AttributeRegion::Cold => self.cold[location.slot()].as_ref(),
        })
    }
}
