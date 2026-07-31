use super::{Aggregator, AttributeId, AttributeLocation};
use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::ModifierSpec;

#[derive(Debug, Clone)]
struct AttributeAggregatorEntry {
    id: AttributeId,
    location: AttributeLocation,
    aggregator: Aggregator,
}

/// Sparse, sorted runtime storage for attribute aggregators.
#[derive(Debug, Clone, Default)]
pub(crate) struct AttributeAggregatorSet {
    entries: Vec<AttributeAggregatorEntry>,
}

impl AttributeAggregatorSet {
    pub(crate) fn get(&self, id: AttributeId) -> Option<&Aggregator> {
        let index = self.search(id).ok()?;
        Some(&self.entries[index].aggregator)
    }

    pub(crate) fn apply_modifier_spec(
        &mut self,
        id: AttributeId,
        location: AttributeLocation,
        spec: &ModifierSpec,
        handle: ActiveEffectHandle,
    ) {
        let index = self.get_or_insert_index(id, location);
        self.entries[index]
            .aggregator
            .apply_modifier_spec(spec, handle);
    }

    pub(crate) fn set_executor(
        &mut self,
        id: AttributeId,
        location: AttributeLocation,
        executor: Option<fn(&Aggregator, f32) -> f32>,
    ) {
        if executor.is_none() {
            return;
        }
        let index = self.get_or_insert_index(id, location);
        self.entries[index].aggregator.set_executor(executor);
    }

    pub(crate) fn remove(&mut self, id: AttributeId) {
        if let Ok(index) = self.search(id) {
            self.entries.remove(index);
        }
    }

    pub(crate) fn remove_modifier_by_handle(
        &mut self,
        id: AttributeId,
        handle: ActiveEffectHandle,
    ) -> bool {
        let Ok(index) = self.search(id) else {
            return false;
        };
        let modifier_count_before = self.entries[index].aggregator.modifier_count();
        self.entries[index]
            .aggregator
            .remove_modifier_by_handle(handle);
        let modifier_count_after = self.entries[index].aggregator.modifier_count();
        let removed = modifier_count_before != modifier_count_after;
        if modifier_count_after == 0 && !self.entries[index].aggregator.has_custom_executor() {
            self.entries.remove(index);
        }
        removed
    }

    pub(crate) fn remove_modifiers_by_handle(
        &mut self,
        handle: ActiveEffectHandle,
        mut on_removed: impl FnMut(AttributeLocation),
    ) {
        self.entries.retain_mut(|entry| {
            let modifier_count_before = entry.aggregator.modifier_count();
            entry.aggregator.remove_modifier_by_handle(handle);
            let modifier_count_after = entry.aggregator.modifier_count();
            if modifier_count_before != modifier_count_after {
                on_removed(entry.location);
            }
            modifier_count_after != 0 || entry.aggregator.has_custom_executor()
        });
    }

    fn get_or_insert_index(&mut self, id: AttributeId, location: AttributeLocation) -> usize {
        match self.search(id) {
            Ok(index) => {
                debug_assert_eq!(self.entries[index].location, location);
                index
            }
            Err(index) => {
                self.entries.insert(
                    index,
                    AttributeAggregatorEntry {
                        id,
                        location,
                        aggregator: Aggregator::default(),
                    },
                );
                index
            }
        }
    }

    fn search(&self, id: AttributeId) -> Result<usize, usize> {
        self.entries
            .binary_search_by_key(&id.to_index(), |entry| entry.id.to_index())
    }
}
