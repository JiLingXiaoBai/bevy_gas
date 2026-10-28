use super::AttributeLocation;
use crate::modifiers::{AppliedModifier, ModifierOperation, ModifierSourceId, ModifierSpec};

pub(super) const DIVISOR_ZERO_TOLERANCE: f32 = 1.0e-8;

/// Evaluates retained modifiers against `base_value` using the default ordering.
///
/// Returns the first retained override when present. Otherwise aggregates each operation
/// separately, then applies base addition, additive multiplication, additive division,
/// compound multiplication, and final addition. Additive multipliers and divisors combine
/// as `1 + sum(value - 1)`. Divisors with absolute value at most `1e-8` act as one.
pub fn default_executor(aggregator: &Aggregator, base_value: f32) -> f32 {
    if let Some(override_value) = aggregator.overrides.first() {
        return override_value.get_value();
    }

    let additive = sum_modifiers(&aggregator.additive, 0.0);
    let multiplier = sum_modifiers(&aggregator.multiplicative_additive, 1.0);
    let mut divisor = sum_modifiers(&aggregator.divisive, 1.0);
    if divisor.abs() <= DIVISOR_ZERO_TOLERANCE {
        divisor = 1.0;
    }
    let compound = aggregator
        .multiplicative
        .iter()
        .fold(1.0, |product, modifier| product * modifier.get_value());
    let final_additive = sum_modifiers(&aggregator.final_additive, 0.0);

    (base_value + additive) * multiplier / divisor * compound + final_additive
}

fn sum_modifiers(modifiers: &[AppliedModifier], bias: f32) -> f32 {
    modifiers
        .iter()
        .fold(bias, |sum, modifier| sum + (modifier.get_value() - bias))
}

/// Collects active modifiers for one attribute and evaluates its current value.
#[derive(Debug, Clone)]
pub struct Aggregator {
    additive: Vec<AppliedModifier>,
    multiplicative_additive: Vec<AppliedModifier>,
    multiplicative: Vec<AppliedModifier>,
    overrides: Vec<AppliedModifier>,
    divisive: Vec<AppliedModifier>,
    final_additive: Vec<AppliedModifier>,
    executor: fn(&Aggregator, f32) -> f32,
    has_custom_executor: bool,
}

impl Default for Aggregator {
    fn default() -> Self {
        Self {
            additive: Vec::new(),
            multiplicative_additive: Vec::new(),
            multiplicative: Vec::new(),
            overrides: Vec::new(),
            divisive: Vec::new(),
            final_additive: Vec::new(),
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
            ModifierOperation::AddBase => self.additive.push(applied_modifier),
            ModifierOperation::MultiplyCompound => self.multiplicative.push(applied_modifier),
            ModifierOperation::MultiplyAdditive => {
                self.multiplicative_additive.push(applied_modifier)
            }
            ModifierOperation::Override => self.overrides.push(applied_modifier),
            ModifierOperation::DivideAdditive => self.divisive.push(applied_modifier),
            ModifierOperation::AddFinal => self.final_additive.push(applied_modifier),
        }
    }

    /// Borrows the retained modifiers for `operation` in current storage order.
    ///
    /// Returns an allocation-free, read-only iterator over values and source IDs for
    /// custom executors. New modifiers are appended; source removal uses swap removal
    /// and can reorder surviving entries. All overrides remain retained, with the first
    /// entry selected by the default executor. Unselected modifiers remain available
    /// until their source is removed. Refreshing a source removes and appends its entries.
    pub fn modifiers(
        &self,
        operation: ModifierOperation,
    ) -> impl DoubleEndedIterator<Item = &AppliedModifier> + ExactSizeIterator + '_ {
        let modifiers = match operation {
            ModifierOperation::AddBase => self.additive.as_slice(),
            ModifierOperation::MultiplyAdditive => self.multiplicative_additive.as_slice(),
            ModifierOperation::MultiplyCompound => self.multiplicative.as_slice(),
            ModifierOperation::Override => self.overrides.as_slice(),
            ModifierOperation::DivideAdditive => self.divisive.as_slice(),
            ModifierOperation::AddFinal => self.final_additive.as_slice(),
        };
        modifiers.iter()
    }

    /// Removes every modifier owned by `source_id` using swap removal.
    ///
    /// Surviving entries can change order, including which override is selected next.
    pub fn remove_modifiers_by_source(&mut self, source_id: ModifierSourceId) {
        for modifiers in [
            &mut self.additive,
            &mut self.multiplicative_additive,
            &mut self.multiplicative,
            &mut self.overrides,
            &mut self.divisive,
            &mut self.final_additive,
        ] {
            let mut index = 0;
            while index < modifiers.len() {
                if modifiers[index].get_source_id() == source_id {
                    modifiers.swap_remove(index);
                } else {
                    index += 1;
                }
            }
        }
    }

    /// Compatibility spelling for [`Self::remove_modifiers_by_source`].
    pub fn remove_modifier_by_handle(&mut self, source_id: impl Into<ModifierSourceId>) {
        self.remove_modifiers_by_source(source_id.into());
    }

    /// Removes all retained modifiers.
    pub fn reset(&mut self) {
        self.additive.clear();
        self.multiplicative_additive.clear();
        self.multiplicative.clear();
        self.overrides.clear();
        self.divisive.clear();
        self.final_additive.clear();
    }

    /// Returns the total number of modifiers across all operation slots.
    pub fn modifier_count(&self) -> usize {
        self.additive.len()
            + self.multiplicative_additive.len()
            + self.multiplicative.len()
            + self.overrides.len()
            + self.divisive.len()
            + self.final_additive.len()
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
