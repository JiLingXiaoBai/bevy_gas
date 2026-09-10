use super::super::aggregation::AttributeAggregatorSet;
use super::super::{
    Aggregator, AttributeId, AttributeIdError, AttributeIdManager, AttributeLocation,
    AttributeRegion, AttributeSnapshot,
};
use crate::modifiers::{ModifierOperation, ModifierSpec};
use crate::settings::GameplayAbilitySystemSettings;
use bevy::prelude::Component;
use std::error::Error;
use std::fmt;

/// Maximum number of registered attributes across both storage regions.
pub const ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::ATTRIBUTE_SET_SIZE;
/// Maximum number of attributes in the hot storage region.
pub const HOT_ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::HOT_ATTRIBUTE_SET_SIZE;
/// Maximum number of attributes in the cold storage region.
pub const COLD_ATTRIBUTE_SET_SIZE: usize = GameplayAbilitySystemSettings::COLD_ATTRIBUTE_SET_SIZE;

pub(super) const HOT_DIRTY_WORDS: usize = HOT_ATTRIBUTE_SET_SIZE.div_ceil(64);
pub(super) const COLD_DIRTY_WORDS: usize = COLD_ATTRIBUTE_SET_SIZE.div_ceil(64);

/// Callback invoked after an instant modifier changes an initialized attribute.
///
/// Cost affordability previews do not invoke this callback or simulate its mutations. It runs
/// once per modifier only during actual execution, after base modification and aggregation.
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

#[derive(Debug, Clone, Copy)]
pub(super) struct Attribute {
    base: f32,
    current: f32,
}

impl Attribute {
    pub(super) const fn new(base_value: f32) -> Self {
        Self {
            base: base_value,
            current: base_value,
        }
    }

    pub(super) fn recalculate(&mut self, aggregator: Option<&Aggregator>) {
        self.current = aggregator.map_or(self.base, |aggregator| aggregator.evaluate(self.base));
    }

    pub(super) const fn get_current_value(&self) -> f32 {
        self.current
    }

    /// Applies the base operation and reevaluates its current value with the same aggregator.
    pub(super) fn apply_instant_modifier(
        &mut self,
        spec: &ModifierSpec,
        aggregator: Option<&Aggregator>,
    ) {
        self.modify_base_value(spec);
        self.recalculate(aggregator);
    }

    fn modify_base_value(&mut self, spec: &ModifierSpec) {
        match spec.get_operation() {
            ModifierOperation::Add => self.base += spec.get_value(),
            ModifierOperation::PercentAdd => self.base *= 1.0 + spec.get_value(),
            ModifierOperation::Multiply => self.base *= spec.get_value(),
            ModifierOperation::Override => self.base = spec.get_value(),
        }
    }

    pub(super) const fn make_snapshot(&self) -> AttributeSnapshot {
        AttributeSnapshot::new(self.base, self.current)
    }
}

/// Per-entity attribute state and its active modifier aggregators.
#[derive(Component)]
pub struct AttributeSet {
    pub(super) hot_attributes: [Option<Attribute>; HOT_ATTRIBUTE_SET_SIZE],
    pub(super) cold_attributes: Box<[Option<Attribute>; COLD_ATTRIBUTE_SET_SIZE]>,
    pub(super) aggregators: AttributeAggregatorSet,
    pub(super) hot_dirty: [u64; HOT_DIRTY_WORDS],
    pub(super) cold_dirty: [u64; COLD_DIRTY_WORDS],
    pub(super) post_execute: Option<AttributePostExecute>,
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
    pub(super) fn attribute_slot_mut(
        &mut self,
        location: AttributeLocation,
    ) -> &mut Option<Attribute> {
        match location.region() {
            AttributeRegion::Hot => &mut self.hot_attributes[location.slot()],
            AttributeRegion::Cold => &mut self.cold_attributes[location.slot()],
        }
    }

    pub(super) fn attribute_slot(&self, location: AttributeLocation) -> &Option<Attribute> {
        match location.region() {
            AttributeRegion::Hot => &self.hot_attributes[location.slot()],
            AttributeRegion::Cold => &self.cold_attributes[location.slot()],
        }
    }

    pub(super) fn initialized_attribute_location(
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

    pub(super) fn mark_dirty(&mut self, location: AttributeLocation) {
        mark_dirty_in_masks(&mut self.hot_dirty, &mut self.cold_dirty, location);
    }

    pub(super) fn take_dirty(&mut self, location: AttributeLocation) -> bool {
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

    pub(super) fn attribute_and_aggregators_mut(
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

pub(super) fn mark_dirty_in_masks<const HOT_WORDS: usize, const COLD_WORDS: usize>(
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
