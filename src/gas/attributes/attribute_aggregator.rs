use crate::gameplay_effects::ActiveEffectHandle;
use crate::modifiers::{AppliedModifier, ModifierOperation, ModifierSpec};

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

#[derive(Debug, Clone)]
pub struct Aggregator {
    additive: Vec<AppliedModifier>,
    percent_additive: Vec<AppliedModifier>,
    multiplicative: Vec<AppliedModifier>,
    override_value: Option<AppliedModifier>,
    executor: fn(&Aggregator, f32) -> f32,
}

impl Default for Aggregator {
    fn default() -> Self {
        Self {
            additive: Vec::new(),
            percent_additive: Vec::new(),
            multiplicative: Vec::new(),
            override_value: None,
            executor: default_executor,
        }
    }
}

impl Aggregator {
    pub fn set_executor(&mut self, executor: Option<fn(&Aggregator, f32) -> f32>) {
        if let Some(executor) = executor {
            self.executor = executor;
        }
    }

    pub fn apply_modifier_spec(&mut self, spec: &ModifierSpec, handle: ActiveEffectHandle) {
        let applied_modifier = AppliedModifier::new(handle, spec.get_value());
        match spec.get_operation() {
            ModifierOperation::Add => self.additive.push(applied_modifier),
            ModifierOperation::Multiply => self.multiplicative.push(applied_modifier),
            ModifierOperation::PercentAdd => self.percent_additive.push(applied_modifier),
            ModifierOperation::Override => self.override_value = Some(applied_modifier),
        }
    }

    pub fn remove_modifier_by_handle(&mut self, handle: ActiveEffectHandle) {
        self.additive
            .retain(|modifier| modifier.get_handle() != handle);
        self.percent_additive
            .retain(|modifier| modifier.get_handle() != handle);
        self.multiplicative
            .retain(|modifier| modifier.get_handle() != handle);
        if let Some(override_modifier) = self.override_value
            && override_modifier.get_handle() == handle
        {
            self.override_value = None;
        }
    }

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

    pub fn evaluate(&self, base_value: f32) -> f32 {
        (self.executor)(self, base_value)
    }
}
