use super::attribute_aggregator::Aggregator;
use super::attribute_snapshot::AttributeSnapshot;
use crate::modifiers::{ModifierOperation, ModifierSpec};

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    base: f32,
    current: f32,
}

impl Attribute {
    pub(crate) fn new(base_value: f32) -> Self {
        Self {
            base: base_value,
            current: base_value,
        }
    }

    pub(crate) fn recalculate(&mut self, aggregator: Option<&Aggregator>) {
        self.current = aggregator.map_or(self.base, |aggregator| aggregator.evaluate(self.base));
    }

    pub(crate) fn get_current_value(&self) -> f32 {
        self.current
    }

    pub(crate) fn modify_base_value(&mut self, spec: &ModifierSpec) {
        match spec.get_operation() {
            ModifierOperation::Add => self.base += spec.get_value(),
            ModifierOperation::PercentAdd => self.base *= 1.0 + spec.get_value(),
            ModifierOperation::Multiply => self.base *= spec.get_value(),
            ModifierOperation::Override => self.base = spec.get_value(),
        }
    }

    pub(crate) fn make_snapshot(&self) -> AttributeSnapshot {
        AttributeSnapshot::new(self.base, self.current)
    }
}
