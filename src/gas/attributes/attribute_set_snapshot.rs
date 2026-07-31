use super::{
    AttributeId, AttributeIdManager, AttributeRegion, AttributeSnapshot, COLD_ATTRIBUTE_SET_SIZE,
    HOT_ATTRIBUTE_SET_SIZE,
};
use bevy::prelude::*;

#[derive(Component, Clone)]
pub struct AttributeSetSnapshot {
    hot: Box<[Option<AttributeSnapshot>; HOT_ATTRIBUTE_SET_SIZE]>,
    cold: Box<[Option<AttributeSnapshot>; COLD_ATTRIBUTE_SET_SIZE]>,
    source_entity: Entity,
}

impl AttributeSetSnapshot {
    pub(crate) fn new(
        hot: Box<[Option<AttributeSnapshot>; HOT_ATTRIBUTE_SET_SIZE]>,
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
    pub fn get_current_value(&self, manager: &AttributeIdManager, id: AttributeId) -> Option<f32> {
        self.get_attribute(manager, id)
            .map(AttributeSnapshot::current)
    }

    /// Returns the captured base value for `id`.
    pub fn get_base_value(&self, manager: &AttributeIdManager, id: AttributeId) -> Option<f32> {
        self.get_attribute(manager, id).map(AttributeSnapshot::base)
    }

    /// Returns the entity from which this snapshot was captured.
    pub fn get_source_entity(&self) -> Entity {
        self.source_entity
    }

    fn get_attribute(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Option<&AttributeSnapshot> {
        let location = manager.location(id)?;
        match location.region() {
            AttributeRegion::Hot => self.hot[location.slot()].as_ref(),
            AttributeRegion::Cold => self.cold[location.slot()].as_ref(),
        }
    }
}
