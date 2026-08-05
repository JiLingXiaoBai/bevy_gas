use super::super::{Aggregator, AttributeId, AttributeIdError, AttributeIdManager};
use super::state::{Attribute, AttributePostExecute, AttributeSet, AttributeSetError};
use crate::modifiers::{ModifierSourceId, ModifierSpec};

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
