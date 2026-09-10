use super::AttributeLocation;
use crate::modifiers::{AppliedModifier, ModifierOperation, ModifierSourceId, ModifierSpec};

/// Evaluates an aggregator using the default operation ordering.
pub fn default_executor(aggregator: &Aggregator, base_value: f32) -> f32 {
    if let Some(override_value) = aggregator.override_value {
        return override_value.get_value();
    }

    let mut final_value = base_value;
    for &add in &aggregator.additive {
        final_value += add.get_value();
    }

    let mut percent_sum = 0.0;
    for &percent in &aggregator.percent_additive {
        percent_sum += percent.get_value();
    }
    final_value *= 1.0 + percent_sum;

    for &multiplier in &aggregator.multiplicative {
        final_value *= multiplier.get_value();
    }

    final_value
}

/// Collects active modifiers for one attribute and evaluates its current value.
#[derive(Debug, Clone)]
pub struct Aggregator {
    additive: Vec<AppliedModifier>,
    percent_additive: Vec<AppliedModifier>,
    multiplicative: Vec<AppliedModifier>,
    override_value: Option<AppliedModifier>,
    executor: fn(&Aggregator, f32) -> f32,
    has_custom_executor: bool,
}

impl Default for Aggregator {
    fn default() -> Self {
        Self {
            additive: Vec::new(),
            percent_additive: Vec::new(),
            multiplicative: Vec::new(),
            override_value: None,
            executor: default_executor,
            has_custom_executor: false,
        }
    }
}

impl Aggregator {
    /// Replaces the value executor when `executor` is present.
    ///
    /// Executors also run during cost affordability previews. They must produce deterministic
    /// values without side effects or dependence on how often they are called.
    pub fn set_executor(&mut self, executor: Option<fn(&Aggregator, f32) -> f32>) {
        if let Some(executor) = executor {
            self.executor = executor;
            self.has_custom_executor = true;
        }
    }

    pub(crate) fn has_custom_executor(&self) -> bool {
        self.has_custom_executor
    }

    /// Retains an evaluated modifier under its runtime source.
    pub fn apply_modifier_spec(
        &mut self,
        spec: &ModifierSpec,
        source_id: impl Into<ModifierSourceId>,
    ) {
        let applied_modifier = AppliedModifier::new(source_id.into(), spec.get_value());
        match spec.get_operation() {
            ModifierOperation::Add => self.additive.push(applied_modifier),
            ModifierOperation::Multiply => self.multiplicative.push(applied_modifier),
            ModifierOperation::PercentAdd => self.percent_additive.push(applied_modifier),
            ModifierOperation::Override => self.override_value = Some(applied_modifier),
        }
    }

    /// Removes every modifier owned by `source_id`.
    pub fn remove_modifiers_by_source(&mut self, source_id: ModifierSourceId) {
        self.additive
            .retain(|modifier| modifier.get_source_id() != source_id);
        self.percent_additive
            .retain(|modifier| modifier.get_source_id() != source_id);
        self.multiplicative
            .retain(|modifier| modifier.get_source_id() != source_id);
        if self
            .override_value
            .is_some_and(|modifier| modifier.get_source_id() == source_id)
        {
            self.override_value = None;
        }
    }

    /// Compatibility spelling for [`Self::remove_modifiers_by_source`].
    pub fn remove_modifier_by_handle(&mut self, source_id: impl Into<ModifierSourceId>) {
        self.remove_modifiers_by_source(source_id.into());
    }

    /// Removes all retained modifiers.
    pub fn reset(&mut self) {
        self.additive.clear();
        self.percent_additive.clear();
        self.multiplicative.clear();
        self.override_value = None;
    }

    /// Returns the total number of modifiers across all operation slots.
    pub fn modifier_count(&self) -> usize {
        self.additive.len()
            + self.percent_additive.len()
            + self.multiplicative.len()
            + usize::from(self.override_value.is_some())
    }

    /// Evaluates the current value from `base_value`.
    pub fn evaluate(&self, base_value: f32) -> f32 {
        (self.executor)(self, base_value)
    }
}

#[derive(Debug, Clone)]
struct AttributeAggregatorEntry {
    location: AttributeLocation,
    aggregator: Aggregator,
}

/// Sparse, sorted runtime storage for attribute aggregators.
#[derive(Debug, Clone, Default)]
pub(super) struct AttributeAggregatorSet {
    entries: Vec<AttributeAggregatorEntry>,
}

impl AttributeAggregatorSet {
    pub(super) fn get(&self, location: AttributeLocation) -> Option<&Aggregator> {
        let index = self.search(location).ok()?;
        Some(&self.entries[index].aggregator)
    }

    pub(super) fn apply_modifier_spec(
        &mut self,
        location: AttributeLocation,
        spec: &ModifierSpec,
        source_id: ModifierSourceId,
    ) {
        let index = self.get_or_insert_index(location);
        self.entries[index]
            .aggregator
            .apply_modifier_spec(spec, source_id);
    }

    pub(super) fn set_executor(
        &mut self,
        location: AttributeLocation,
        executor: Option<fn(&Aggregator, f32) -> f32>,
    ) {
        if executor.is_none() {
            return;
        }
        let index = self.get_or_insert_index(location);
        self.entries[index].aggregator.set_executor(executor);
    }

    pub(super) fn remove(&mut self, location: AttributeLocation) {
        if let Ok(index) = self.search(location) {
            self.entries.remove(index);
        }
    }

    pub(super) fn remove_modifier_by_source(
        &mut self,
        location: AttributeLocation,
        source_id: ModifierSourceId,
    ) -> bool {
        let Ok(index) = self.search(location) else {
            return false;
        };
        let modifier_count_before = self.entries[index].aggregator.modifier_count();
        self.entries[index]
            .aggregator
            .remove_modifiers_by_source(source_id);
        let modifier_count_after = self.entries[index].aggregator.modifier_count();
        let removed = modifier_count_before != modifier_count_after;
        if modifier_count_after == 0 && !self.entries[index].aggregator.has_custom_executor() {
            self.entries.remove(index);
        }
        removed
    }

    pub(super) fn remove_modifiers_by_source(
        &mut self,
        source_id: ModifierSourceId,
        mut on_removed: impl FnMut(AttributeLocation),
    ) {
        self.entries.retain_mut(|entry| {
            let modifier_count_before = entry.aggregator.modifier_count();
            entry.aggregator.remove_modifiers_by_source(source_id);
            let modifier_count_after = entry.aggregator.modifier_count();
            if modifier_count_before != modifier_count_after {
                on_removed(entry.location);
            }
            modifier_count_after != 0 || entry.aggregator.has_custom_executor()
        });
    }

    fn get_or_insert_index(&mut self, location: AttributeLocation) -> usize {
        match self.search(location) {
            Ok(index) => index,
            Err(index) => {
                self.entries.insert(
                    index,
                    AttributeAggregatorEntry {
                        location,
                        aggregator: Aggregator::default(),
                    },
                );
                index
            }
        }
    }

    fn search(&self, location: AttributeLocation) -> Result<usize, usize> {
        self.entries
            .binary_search_by_key(&location.sort_key(), |entry| entry.location.sort_key())
    }
}
