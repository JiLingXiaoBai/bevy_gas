use super::super::aggregation::AttributeAggregatorSet;
use super::super::{
    AttributeId, AttributeIdError, AttributeIdManager, AttributeLocation, AttributeRegion,
    AttributeSetSnapshot,
};
use super::state::{Attribute, AttributeSet};
use bevy::prelude::{Changed, Entity, Query};

impl AttributeSet {
    /// Recalculates one initialized attribute if its dirty bit is set.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered.
    pub fn recalculate_attribute(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<(), AttributeIdError> {
        let location = manager.location(id)?;
        self.recalculate_location(location);
        Ok(())
    }

    /// Recalculates all attributes selected by the hot and cold dirty masks.
    pub fn recalculate_dirty(&mut self) {
        recalculate_region(
            &mut self.hot_attributes,
            &mut self.hot_dirty,
            &self.aggregators,
            AttributeRegion::Hot,
        );
        recalculate_region(
            &mut self.cold_attributes,
            &mut self.cold_dirty,
            &self.aggregators,
            AttributeRegion::Cold,
        );
    }

    /// Returns the current value for an initialized attribute.
    ///
    /// `Ok(None)` means the ID is valid but this set has not initialized it.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered.
    pub fn get_current_value(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<Option<f32>, AttributeIdError> {
        let location = manager.location(id)?;
        let was_dirty = self.take_dirty(location);
        let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
        let Some(attribute) = attribute.as_mut() else {
            return Ok(None);
        };
        if was_dirty {
            attribute.recalculate(aggregators.get(location));
        }
        Ok(Some(attribute.get_current_value()))
    }

    /// Captures the current hot and cold attribute values for `source_entity`.
    pub fn make_snapshot(&mut self, source_entity: Entity) -> AttributeSetSnapshot {
        self.recalculate_dirty();

        let hot = std::array::from_fn(|index| {
            self.hot_attributes[index]
                .as_ref()
                .map(Attribute::make_snapshot)
        });
        let cold = Box::new(std::array::from_fn(|index| {
            self.cold_attributes[index]
                .as_ref()
                .map(Attribute::make_snapshot)
        }));

        AttributeSetSnapshot::new(hot, cold, source_entity)
    }

    fn recalculate_location(&mut self, location: AttributeLocation) {
        if self.take_dirty(location) {
            let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
            if let Some(attribute) = attribute {
                attribute.recalculate(aggregators.get(location));
            }
        }
    }
}

fn recalculate_region<const SIZE: usize, const WORDS: usize>(
    attributes: &mut [Option<Attribute>; SIZE],
    dirty: &mut [u64; WORDS],
    aggregators: &AttributeAggregatorSet,
    region: AttributeRegion,
) {
    for (word_index, dirty_word) in dirty.iter_mut().enumerate() {
        let mut bits = std::mem::take(dirty_word);
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let index = word_index * 64 + bit;
            if let Some(attribute) = attributes.get_mut(index).and_then(Option::as_mut) {
                let location = AttributeLocation::new(region, index);
                attribute.recalculate(aggregators.get(location));
            }
            bits &= bits - 1;
        }
    }
}

/// Recalculates dirty attributes on changed attribute-set components.
pub fn recalculate_attribute_sets_system(
    mut query: Query<&mut AttributeSet, Changed<AttributeSet>>,
) {
    for mut attribute_set in &mut query {
        attribute_set.recalculate_dirty();
    }
}
