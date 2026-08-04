use super::attribute::Attribute;
use super::attribute_aggregator_set::AttributeAggregatorSet;
use super::{
    Aggregator, AttributeId, AttributeIdError, AttributeIdManager, AttributeLocation,
    AttributeRegion, AttributeSetSnapshot,
};
use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::ModifierSpec;
use crate::settings::GameplayAbilitySystemSettings;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;

/// Maximum number of registered attributes across both storage regions.
pub const ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::ATTRIBUTE_SET_SIZE;
/// Maximum number of attributes in the hot storage region.
pub const HOT_ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::HOT_ATTRIBUTE_SET_SIZE;
/// Maximum number of attributes in the cold storage region.
pub const COLD_ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::COLD_ATTRIBUTE_SET_SIZE;

const HOT_DIRTY_WORDS: usize = HOT_ATTRIBUTE_SET_SIZE.div_ceil(64);
const COLD_DIRTY_WORDS: usize = COLD_ATTRIBUTE_SET_SIZE.div_ceil(64);

/// Callback invoked after an instant modifier changes an initialized attribute.
pub type AttributePostExecute = fn(&mut AttributeSet, &AttributeIdManager, AttributeId, f32, f32);

/// Describes why an operation on an [`AttributeSet`] could not be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeSetError {
    /// The attribute ID is invalid for the supplied manager.
    AttributeId(AttributeIdError),
    /// The attribute ID is valid, but this set has not initialized its slot.
    UninitializedAttribute { id: AttributeId },
}

impl fmt::Display for AttributeSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttributeId(error) => write!(f, "attribute lookup failed: {error}"),
            Self::UninitializedAttribute { id } => write!(
                f,
                "attribute {} is not initialized in this AttributeSet",
                id.to_index()
            ),
        }
    }
}

impl Error for AttributeSetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AttributeId(error) => Some(error),
            Self::UninitializedAttribute { .. } => None,
        }
    }
}

impl From<AttributeIdError> for AttributeSetError {
    fn from(value: AttributeIdError) -> Self {
        Self::AttributeId(value)
    }
}

#[derive(Component)]
pub struct AttributeSet {
    hot_attributes: [Option<Attribute>; HOT_ATTRIBUTE_SET_SIZE],
    cold_attributes: Box<[Option<Attribute>; COLD_ATTRIBUTE_SET_SIZE]>,
    aggregators: AttributeAggregatorSet,
    hot_dirty: [u64; HOT_DIRTY_WORDS],
    cold_dirty: [u64; COLD_DIRTY_WORDS],
    post_execute: Option<AttributePostExecute>,
}

impl Default for AttributeSet {
    fn default() -> Self {
        Self {
            hot_attributes: std::array::from_fn(|_| None),
            cold_attributes: Box::new(std::array::from_fn(|_| None)),
            aggregators: AttributeAggregatorSet::default(),
            hot_dirty: [0; HOT_DIRTY_WORDS],
            cold_dirty: [0; COLD_DIRTY_WORDS],
            post_execute: None,
        }
    }
}

impl AttributeSet {
    /// Initializes the storage slot assigned to `id` by the global manager.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] if `id` is not registered
    /// in `manager`.
    pub fn initialize_attribute(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
        base_value: f32,
        executor: Option<fn(&Aggregator, f32) -> f32>,
    ) -> Result<(), AttributeIdError> {
        let location = manager.location(id)?;
        self.aggregators.remove(location);
        self.aggregators.set_executor(location, executor);
        *self.attribute_slot_mut(location) = Some(Attribute::new(base_value));
        self.mark_dirty(location);
        Ok(())
    }

    /// Sets the callback invoked after instant modifier execution.
    pub fn set_post_execute(&mut self, post_execute: Option<AttributePostExecute>) {
        self.post_execute = post_execute;
    }

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

    /// Applies an instant modifier and invokes the post-execute callback.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeSetError::AttributeId`] if the modifier attribute is
    /// not registered, or [`AttributeSetError::UninitializedAttribute`] if this
    /// set has not initialized it.
    pub fn apply_instant_modifier(
        &mut self,
        manager: &AttributeIdManager,
        spec: &ModifierSpec,
    ) -> Result<(), AttributeSetError> {
        let id = spec.get_id();
        let location = self.initialized_attribute_location(manager, id)?;
        let was_dirty = self.take_dirty(location);
        let old_value = {
            let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
            let Some(attribute) = attribute.as_mut() else {
                return Err(AttributeSetError::UninitializedAttribute { id });
            };
            if was_dirty {
                attribute.recalculate(aggregators.get(location));
            }
            let old_value = attribute.get_current_value();
            attribute.modify_base_value(spec);
            old_value
        };
        self.mark_dirty(location);

        let Some(new_value) = self.get_current_value(manager, id)? else {
            return Err(AttributeSetError::UninitializedAttribute { id });
        };
        if let Some(post_execute) = self.post_execute {
            post_execute(self, manager, id, old_value, new_value);
        }
        Ok(())
    }

    /// Applies a duration modifier associated with an active effect handle.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeSetError::AttributeId`] if the modifier attribute is
    /// not registered, or [`AttributeSetError::UninitializedAttribute`] if this
    /// set has not initialized it.
    pub fn apply_duration_modifier(
        &mut self,
        manager: &AttributeIdManager,
        spec: &ModifierSpec,
        handle: ActiveEffectHandle,
    ) -> Result<(), AttributeSetError> {
        let location = self.initialized_attribute_location(manager, spec.get_id())?;
        self.aggregators.apply_modifier_spec(location, spec, handle);
        self.mark_dirty(location);
        Ok(())
    }

    /// Verifies that `id` is registered and initialized in this set.
    pub(crate) fn validate_initialized_attribute(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<(), AttributeSetError> {
        self.initialized_attribute_location(manager, id).map(drop)
    }

    /// Removes modifiers with `handle` from every initialized attribute.
    pub fn remove_modifiers(&mut self, handle: ActiveEffectHandle) {
        let (hot_dirty, cold_dirty) = (&mut self.hot_dirty, &mut self.cold_dirty);
        self.aggregators
            .remove_modifiers_by_handle(handle, |location| {
                mark_dirty_in_masks(hot_dirty, cold_dirty, location);
            });
    }

    /// Removes modifiers with `handle` from the supplied attribute IDs.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] before mutation if any ID
    /// is not registered.
    pub fn remove_modifiers_for_attributes(
        &mut self,
        manager: &AttributeIdManager,
        handle: ActiveEffectHandle,
        ids: impl IntoIterator<Item = AttributeId>,
    ) -> Result<(), AttributeIdError> {
        let locations = ids
            .into_iter()
            .map(|id| manager.location(id))
            .collect::<Result<Vec<_>, _>>()?;
        for location in locations {
            let removed = self.aggregators.remove_modifier_by_handle(location, handle);
            if removed {
                self.mark_dirty(location);
            }
        }
        Ok(())
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

    fn attribute_slot_mut(&mut self, location: AttributeLocation) -> &mut Option<Attribute> {
        match location.region() {
            AttributeRegion::Hot => &mut self.hot_attributes[location.slot()],
            AttributeRegion::Cold => &mut self.cold_attributes[location.slot()],
        }
    }

    fn attribute_slot(&self, location: AttributeLocation) -> &Option<Attribute> {
        match location.region() {
            AttributeRegion::Hot => &self.hot_attributes[location.slot()],
            AttributeRegion::Cold => &self.cold_attributes[location.slot()],
        }
    }

    fn initialized_attribute_location(
        &self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Result<AttributeLocation, AttributeSetError> {
        let location = manager.location(id)?;
        if self.attribute_slot(location).is_none() {
            return Err(AttributeSetError::UninitializedAttribute { id });
        }
        Ok(location)
    }

    fn mark_dirty(&mut self, location: AttributeLocation) {
        mark_dirty_in_masks(&mut self.hot_dirty, &mut self.cold_dirty, location);
    }

    fn take_dirty(&mut self, location: AttributeLocation) -> bool {
        let (word, bit) = (location.slot() / 64, location.slot() % 64);
        let dirty_word = match location.region() {
            AttributeRegion::Hot => &mut self.hot_dirty[word],
            AttributeRegion::Cold => &mut self.cold_dirty[word],
        };
        let mask = 1 << bit;
        let was_dirty = *dirty_word & mask != 0;
        *dirty_word &= !mask;
        was_dirty
    }

    fn recalculate_location(&mut self, location: AttributeLocation) {
        if self.take_dirty(location) {
            let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
            if let Some(attribute) = attribute {
                attribute.recalculate(aggregators.get(location));
            }
        }
    }

    fn attribute_and_aggregators_mut(
        &mut self,
        location: AttributeLocation,
    ) -> (&mut Option<Attribute>, &AttributeAggregatorSet) {
        let aggregators = &self.aggregators;
        let attribute = match location.region() {
            AttributeRegion::Hot => &mut self.hot_attributes[location.slot()],
            AttributeRegion::Cold => &mut self.cold_attributes[location.slot()],
        };
        (attribute, aggregators)
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

fn mark_dirty_in_masks<const HOT_WORDS: usize, const COLD_WORDS: usize>(
    hot_dirty: &mut [u64; HOT_WORDS],
    cold_dirty: &mut [u64; COLD_WORDS],
    location: AttributeLocation,
) {
    let (word, bit) = (location.slot() / 64, location.slot() % 64);
    match location.region() {
        AttributeRegion::Hot => hot_dirty[word] |= 1 << bit,
        AttributeRegion::Cold => cold_dirty[word] |= 1 << bit,
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
