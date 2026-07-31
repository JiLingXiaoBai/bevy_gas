use super::attribute::Attribute;
use super::*;
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
    hot_dirty: [u64; HOT_DIRTY_WORDS],
    cold_dirty: [u64; COLD_DIRTY_WORDS],
    post_execute: Option<AttributePostExecute>,
}

impl Default for AttributeSet {
    fn default() -> Self {
        Self {
            hot_attributes: Box::new(std::array::from_fn(|_| None)),
            cold_attributes: Box::new(std::array::from_fn(|_| None)),
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
        let mut attribute = Attribute::default();
        attribute.init(base_value, executor);
        *self.attribute_slot_mut(location) = Some(attribute);
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
        recalculate_region(&mut self.hot_attributes, &mut self.hot_dirty);
        recalculate_region(&mut self.cold_attributes, &mut self.cold_dirty);
    }

    /// Returns the current value for an initialized attribute.
    pub fn get_current_value(
        &mut self,
        manager: &AttributeIdManager,
        id: AttributeId,
    ) -> Option<f32> {
        let location = manager.location(id)?;
        let was_dirty = self.take_dirty(location);
        let attribute = self.attribute_slot_mut(location).as_mut()?;
        if was_dirty {
            attribute.recalculate();
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
        if let Some(attribute) = self.attribute_slot_mut(location) {
            attribute.apply_modifier_spec(spec, handle);
            self.mark_dirty(location);
        }
    }

    /// Removes modifiers with `handle` from every initialized attribute.
    pub fn remove_modifiers(&mut self, handle: ActiveEffectHandle) {
        remove_modifiers_from_region(&mut self.hot_attributes, &mut self.hot_dirty, handle);
        remove_modifiers_from_region(&mut self.cold_attributes, &mut self.cold_dirty, handle);
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
            let removed = if let Some(attribute) = self.attribute_slot_mut(location) {
                let len_before = attribute.modifier_count();
                attribute.remove_modifier_by_handle(handle);
                attribute.modifier_count() != len_before
            } else {
                false
            };
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
        let (word, bit) = (location.slot() / 64, location.slot() % 64);
        match location.region() {
            AttributeRegion::Hot => self.hot_dirty[word] |= 1 << bit,
            AttributeRegion::Cold => self.cold_dirty[word] |= 1 << bit,
        }
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
        if self.take_dirty(location)
            && let Some(attribute) = self.attribute_slot_mut(location)
        {
            attribute.recalculate();
        }
    }
}

fn recalculate_region<const SIZE: usize, const WORDS: usize>(
    attributes: &mut [Option<Attribute>; SIZE],
    dirty: &mut [u64; WORDS],
) {
    for (word_index, dirty_word) in dirty.iter_mut().enumerate() {
        let mut bits = std::mem::take(dirty_word);
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let index = word_index * 64 + bit;
            if let Some(attribute) = attributes.get_mut(index).and_then(Option::as_mut) {
                attribute.recalculate();
            }
            bits &= bits - 1;
        }
    }
}

fn remove_modifiers_from_region<const SIZE: usize, const WORDS: usize>(
    attributes: &mut [Option<Attribute>; SIZE],
    dirty: &mut [u64; WORDS],
    handle: ActiveEffectHandle,
) {
    for (index, attribute) in attributes.iter_mut().enumerate() {
        let Some(attribute) = attribute else {
            continue;
        };
        let len_before = attribute.modifier_count();
        attribute.remove_modifier_by_handle(handle);
        if attribute.modifier_count() != len_before {
            dirty[index / 64] |= 1 << (index % 64);
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
