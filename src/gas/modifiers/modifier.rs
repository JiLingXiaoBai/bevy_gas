use crate::attributes::AttributeId;
use crate::gameplay_effects::{ActiveEffectHandle, EffectContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierOperation {
    Add,
    PercentAdd,
    Multiply,
    Override,
}

pub enum ModifierMagnitude {
    Flat(f32),
    Calculated(Box<dyn ModifierMagnitudeCalculation>),
}

pub trait ModifierMagnitudeCalculation: Send + Sync {
    fn calculate(&self, context: &EffectContext) -> f32;
}

pub struct Modifier {
    id: AttributeId,
    op: ModifierOperation,
    magnitude: ModifierMagnitude,
}

impl Modifier {
    pub fn new(id: AttributeId, op: ModifierOperation, magnitude: ModifierMagnitude) -> Self {
        Modifier { id, op, magnitude }
    }

    pub fn get_operation(&self) -> ModifierOperation {
        self.op
    }

    pub fn make_spec(&self, context: &EffectContext) -> ModifierSpec {
        let final_value = match &self.magnitude {
            ModifierMagnitude::Flat(value) => *value,
            ModifierMagnitude::Calculated(calc) => calc.calculate(context),
        };

        ModifierSpec {
            id: self.id,
            op: self.op,
            value: final_value,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModifierSpec {
    id: AttributeId,
    op: ModifierOperation,
    value: f32,
}

impl ModifierSpec {
    pub fn get_id(&self) -> AttributeId {
        self.id
    }

    pub fn get_operation(&self) -> ModifierOperation {
        self.op
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }

    pub fn scaled_by_stack(&self, stack_count: u32) -> Self {
        Self {
            id: self.id,
            op: self.op,
            value: self.value * stack_count as f32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AppliedModifier {
    handle: ActiveEffectHandle,
    value: f32,
}

impl AppliedModifier {
    pub fn new(handle: ActiveEffectHandle, value: f32) -> Self {
        AppliedModifier { handle, value }
    }

    pub fn get_handle(&self) -> ActiveEffectHandle {
        self.handle
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }
}
