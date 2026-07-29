use super::attribute_aggregator::Aggregator;
use super::attribute_snapshot::AttributeSnapshot;
use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::{ModifierOperation, ModifierSpec};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AttributeClamp {
    #[default]
    None,
    Range {
        min: Option<f64>,
        max: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    base: f64,
    evaluated: f64,
    current: f64,
    aggregator: Aggregator,
    clamp: AttributeClamp,
}

impl Default for Attribute {
    fn default() -> Self {
        Self {
            base: 0.0,
            evaluated: 0.0,
            current: 0.0,
            aggregator: Aggregator::default(),
            clamp: AttributeClamp::None,
        }
    }
}

impl Attribute {
    pub(crate) fn init(
        &mut self,
        base_value: f64,
        executor: Option<fn(&Aggregator, f64) -> f64>,
        clamp: AttributeClamp,
    ) {
        self.base = base_value;
        self.clamp = clamp;
        self.set_executor(executor);
    }

    pub(crate) fn recalculate(&mut self) {
        self.evaluated = self.aggregator.evaluate(self.base);
        self.clamp_current();
    }

    pub(crate) fn get_current_value(&self) -> f64 {
        self.current
    }

    pub(crate) fn set_clamp(&mut self, clamp: AttributeClamp) {
        self.clamp = clamp;
    }

    fn clamp_current(&mut self) {
        let (min, max) = self.clamp_bounds();
        let mut value = self.evaluated;
        if let Some(min) = min {
            value = value.max(min);
        }
        if let Some(max) = max {
            value = value.min(max);
        }
        self.current = value;
    }

    pub(crate) fn set_executor(&mut self, executor: Option<fn(&Aggregator, f64) -> f64>) {
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

    fn clamp_bounds(&self) -> (Option<f64>, Option<f64>) {
        match self.clamp {
            AttributeClamp::None => (None, None),
            AttributeClamp::Range { min, max } => (min, max),
        }
    }
}
