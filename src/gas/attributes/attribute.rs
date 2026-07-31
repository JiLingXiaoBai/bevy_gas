use super::attribute_aggregator::Aggregator;
use super::attribute_snapshot::AttributeSnapshot;
use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::{ModifierOperation, ModifierSpec};

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    base: f32,
    current: f32,
    aggregator: Aggregator,
}

impl Default for Attribute {
    fn default() -> Self {
        Self {
            base: 0.0,
            current: 0.0,
            aggregator: Aggregator::default(),
        }
    }
}

impl Attribute {
    pub(crate) fn init(&mut self, base_value: f32, executor: Option<fn(&Aggregator, f32) -> f32>) {
        self.base = base_value;
        self.set_executor(executor);
    }

    pub(crate) fn recalculate(&mut self) {
        self.current = self.aggregator.evaluate(self.base);
    }

    pub(crate) fn get_current_value(&self) -> f32 {
        self.current
    }

    pub(crate) fn set_executor(&mut self, executor: Option<fn(&Aggregator, f32) -> f32>) {
        self.aggregator.set_executor(executor);
    }

    pub(crate) fn apply_modifier_spec(&mut self, spec: &ModifierSpec, handle: ActiveEffectHandle) {
        self.aggregator.apply_modifier_spec(spec, handle);
    }

    pub(crate) fn remove_modifier_by_handle(&mut self, handle: ActiveEffectHandle) {
        self.aggregator.remove_modifier_by_handle(handle);
    }

    pub(crate) fn modify_base_value(&mut self, spec: &ModifierSpec) {
        match spec.get_operation() {
            ModifierOperation::Add => self.base += spec.get_value(),
            ModifierOperation::PercentAdd => self.base *= 1.0 + spec.get_value(),
            ModifierOperation::Multiply => self.base *= spec.get_value(),
            ModifierOperation::Override => self.base = spec.get_value(),
        }
    }

    /// Returns the total number of modifiers applied to this attribute.
    pub(crate) fn modifier_count(&self) -> usize {
        self.aggregator.modifier_count()
    }

    pub(crate) fn make_snapshot(&self) -> AttributeSnapshot {
        AttributeSnapshot::new(self.base, self.current)
    }
}
