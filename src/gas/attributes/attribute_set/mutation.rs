use super::super::{
    Aggregator, AttributeId, AttributeIdError, AttributeIdManager, AttributeRegion,
    AttributeSnapshot,
};
use super::state::{
    Attribute, AttributePostExecute, AttributeSet, AttributeSetError, COLD_ATTRIBUTE_SET_SIZE,
    HOT_ATTRIBUTE_SET_SIZE,
};
use crate::modifiers::{ModifierSourceId, ModifierSpec};
use std::borrow::Cow;

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
        let (old_value, new_value) = {
            let (attribute, aggregators) = self.attribute_and_aggregators_mut(location);
            let Some(attribute) = attribute.as_mut() else {
                return Err(AttributeSetError::UninitializedAttribute { id });
            };
            if was_dirty {
                attribute.recalculate(aggregators.get(location));
            }
            let old_value = attribute.get_current_value();
            attribute.apply_instant_modifier(spec, aggregators.get(location));
            (old_value, attribute.get_current_value())
        };
        if let Some(post_execute) = self.post_execute {
            post_execute(self, manager, id, old_value, new_value);
        }
        Ok(())
    }

    /// Previews ordered instant modifiers against temporary attribute values.
    ///
    /// Removed sources are excluded from the temporary aggregators before evaluation. Each
    /// modified attribute retains its projected base across repeated entries. The predicate
    /// observes every recalculated value before any post-execute callback.
    ///
    /// This borrows the current aggregators when no sources are removed. It does not modify
    /// live values, dirty flags, or callbacks; custom aggregator executors must be pure.
    pub(crate) fn preview_instant_modifiers(
        &self,
        manager: &AttributeIdManager,
        modifiers: &[ModifierSpec],
        removed_sources: impl IntoIterator<Item = ModifierSourceId>,
        mut accepts: impl FnMut(AttributeSnapshot) -> bool,
    ) -> Result<bool, AttributeSetError> {
        let mut aggregators = Cow::Borrowed(&self.aggregators);
        for source in removed_sources {
            aggregators
                .to_mut()
                .remove_modifiers_by_source(source, |_| {});
        }

        let mut hot: [Option<Attribute>; HOT_ATTRIBUTE_SET_SIZE] = [None; HOT_ATTRIBUTE_SET_SIZE];
        let mut cold: [Option<Attribute>; COLD_ATTRIBUTE_SET_SIZE] =
            [None; COLD_ATTRIBUTE_SET_SIZE];
        for modifier in modifiers {
            let id = modifier.get_id();
            let location = self.initialized_attribute_location(manager, id)?;
            let attribute = self
                .attribute_slot(location)
                .as_ref()
                .ok_or(AttributeSetError::UninitializedAttribute { id })?;
            let projected_slot = match location.region() {
                AttributeRegion::Hot => &mut hot[location.slot()],
                AttributeRegion::Cold => &mut cold[location.slot()],
            };
            let projected = projected_slot.get_or_insert(*attribute);
            projected.apply_instant_modifier(modifier, aggregators.get(location));
            if !accepts(projected.make_snapshot()) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Applies a duration modifier associated with a runtime source.
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
        source_id: impl Into<ModifierSourceId>,
    ) -> Result<(), AttributeSetError> {
        let location = self.initialized_attribute_location(manager, spec.get_id())?;
        self.aggregators
            .apply_modifier_spec(location, spec, source_id.into());
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

    /// Removes modifiers owned by `source_id` from every initialized attribute.
    pub fn remove_modifiers(&mut self, source_id: impl Into<ModifierSourceId>) {
        let source_id = source_id.into();
        let (hot_dirty, cold_dirty) = (&mut self.hot_dirty, &mut self.cold_dirty);
        self.aggregators
            .remove_modifiers_by_source(source_id, |location| {
                super::state::mark_dirty_in_masks(hot_dirty, cold_dirty, location);
            });
    }

    /// Removes modifiers owned by `source_id` from the supplied attributes.
    ///
    /// # Errors
    ///
    /// Returns [`AttributeIdError::MissingLocation`] before mutation if any ID
    /// is not registered.
    pub fn remove_modifiers_for_attributes(
        &mut self,
        manager: &AttributeIdManager,
        source_id: impl Into<ModifierSourceId>,
        ids: impl IntoIterator<Item = AttributeId>,
    ) -> Result<(), AttributeIdError> {
        let source_id = source_id.into();
        let locations = ids
            .into_iter()
            .map(|id| manager.location(id))
            .collect::<Result<Vec<_>, _>>()?;
        for location in locations {
            let removed = self
                .aggregators
                .remove_modifier_by_source(location, source_id);
            if removed {
                self.mark_dirty(location);
            }
        }
        Ok(())
    }
}
