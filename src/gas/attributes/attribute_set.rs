use super::attribute::Attribute;
use super::attribute_aggregator_set::AttributeAggregatorSet;
use super::{
    Aggregator, AttributeId, AttributeIdManager, AttributeLocation, AttributeRegion,
    AttributeSetSnapshot,
};
use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::ModifierSpec;
use crate::settings::GameplayAbilitySystemSettings;
use bevy::prelude::*;

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

#[derive(Component)]
pub struct AttributeSet {
    hot_attributes: Box<[Option<Attribute>; HOT_ATTRIBUTE_SET_SIZE]>,
    cold_attributes: Box<[Option<Attribute>; COLD_ATTRIBUTE_SET_SIZE]>,
    aggregators: AttributeAggregatorSet,
    hot_dirty: [u64; HOT_DIRTY_WORDS],
    cold_dirty: [u64; COLD_DIRTY_WORDS],
    post_execute: Option<AttributePostExecute>,
}

impl Default for AttributeSet {
    fn default() -> Self {
        Self {
            hot_attributes: Box::new(std::array::from_fn(|_| None)),
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
    pub fn initialize_attribute(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
        base_value: f32,
        executor: Option<fn(&Aggregator, f32) -> f32>,
    ) {
        let Some(location) = manager.location(id) else {
            debug_assert!(false, "attribute ID is missing from the global manager");
            return;
        };
        self.aggregators.remove(id);
        self.aggregators.set_executor(id, location, executor);
        *self.attribute_slot_mut(location) = Some(Attribute::new(id, base_value));
        self.mark_dirty(location);
    }

    /// Sets the callback invoked after instant modifier execution.
    pub fn set_post_execute(&mut self, post_execute: Option<AttributePostExecute>) {
        self.post_execute = post_execute;
    }

    /// Recalculates one initialized attribute if its dirty bit is set.
    pub fn recalculate_attribute(&mut self, manager: &AttributeIdManager, id: AttributeId) {
        let Some(location) = manager.location(id) else {
            debug_assert!(false, "attribute ID is missing from the global manager");
            return;
        };
        self.recalculate_location(location);
    }

    /// Recalculates all attributes selected by the hot and cold dirty masks.
    pub fn recalculate_dirty(&mut self) {
        recalculate_region(
            &mut self.hot_attributes,
            &mut self.hot_dirty,
            &self.aggregators,
        );
        recalculate_region(
            &mut self.cold_attributes,
            &mut self.cold_dirty,
            &self.aggregators,
        );
    }

    /// Returns the current value for an initialized attribute.
    pub fn get_current_value(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Option<f32> {
        let location = manager.location(id)?;
        let was_dirty = self.take_dirty(location);
        let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
        let attribute = attribute.as_mut()?;
        if was_dirty {
            attribute.recalculate(aggregators.get(attribute.id()));
        }
        Some(attribute.get_current_value())
    }

    /// Applies an instant modifier and invokes the post-execute callback.
    pub fn apply_instant_modifier(&mut self, manager: &AttributeIdManager, spec: &ModifierSpec) {
        let id = spec.get_id();
        let Some(location) = manager.location(id) else {
            debug_assert!(false, "attribute ID is missing from the global manager");
            return;
        };
        let old_value = self.get_current_value(manager, id);
        if let Some(attribute) = self.attribute_slot_mut(location) {
            attribute.modify_base_value(spec);
            self.mark_dirty(location);
        }

        if let (Some(old_value), Some(new_value), Some(post_execute)) = (
            old_value,
            self.get_current_value(manager, id),
            self.post_execute,
        ) {
            post_execute(self, manager, id, old_value, new_value);
        }
    }

    /// Applies a duration modifier associated with an active effect handle.
    pub fn apply_duration_modifier(
        &mut self,
        manager: &AttributeIdManager,
        spec: &ModifierSpec,
        handle: ActiveEffectHandle,
    ) {
        let Some(location) = manager.location(spec.get_id()) else {
            debug_assert!(false, "attribute ID is missing from the global manager");
            return;
        };
        if self.attribute_slot_mut(location).is_some() {
            self.aggregators
                .apply_modifier_spec(spec.get_id(), location, spec, handle);
            self.mark_dirty(location);
        }
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
    pub fn remove_modifiers_for_attributes(
        &mut self,
        manager: &AttributeIdManager,
        handle: ActiveEffectHandle,
        ids: impl IntoIterator<Item = AttributeId>,
    ) {
        for id in ids {
            let Some(location) = manager.location(id) else {
                debug_assert!(false, "attribute ID is missing from the global manager");
                continue;
            };
            let removed = self.aggregators.remove_modifier_by_handle(id, handle);
            if removed {
                self.mark_dirty(location);
            }
        }
    }

    /// Captures the current hot and cold attribute values for `source_entity`.
    pub fn make_snapshot(&mut self, source_entity: Entity) -> AttributeSetSnapshot {
        self.recalculate_dirty();

        let hot = Box::new(std::array::from_fn(|index| {
            self.hot_attributes[index]
                .as_ref()
                .map(Attribute::make_snapshot)
        }));
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
                attribute.recalculate(aggregators.get(attribute.id()));
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
) {
    for (word_index, dirty_word) in dirty.iter_mut().enumerate() {
        let mut bits = std::mem::take(dirty_word);
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let index = word_index * 64 + bit;
            if let Some(attribute) = attributes.get_mut(index).and_then(Option::as_mut) {
                attribute.recalculate(aggregators.get(attribute.id()));
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
