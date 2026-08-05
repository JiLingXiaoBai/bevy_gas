use super::{ModifierEvaluationContext, ModifierSpec};
use crate::attributes::AttributeId;

/// Operation used to combine a modifier with an attribute value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierOperation {
    /// Adds the evaluated magnitude to the value.
    Add,
    /// Adds the evaluated magnitude to the shared percentage multiplier.
    PercentAdd,
    /// Multiplies the value by the evaluated magnitude.
    Multiply,
    /// Replaces the evaluated value while the modifier is active.
    Override,
}

/// Describes how a modifier obtains its evaluated magnitude.
pub enum ModifierMagnitude {
    /// Uses a constant value.
    Flat(f32),
    /// Evaluates a custom calculation against a read-only context.
    Calculated(Box<dyn ModifierMagnitudeCalculation>),
}

/// Calculates a modifier magnitude from context supplied by the caller.
pub trait ModifierMagnitudeCalculation: Send + Sync {
    /// Evaluates and returns the modifier magnitude.
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32;
}

/// Immutable definition of a modification targeting one attribute.
pub struct Modifier {
    id: AttributeId,
    op: ModifierOperation,
    magnitude: ModifierMagnitude,
}

impl Modifier {
    /// Creates a modifier definition.
    pub fn new(id: AttributeId, op: ModifierOperation, magnitude: ModifierMagnitude) -> Self {
        Self { id, op, magnitude }
    }

    /// Returns the operation used by this modifier.
    pub fn get_operation(&self) -> ModifierOperation {
        self.op
    }

    /// Evaluates this definition into a value-only specification.
    pub fn make_spec(&self, context: &dyn ModifierEvaluationContext) -> ModifierSpec {
        let final_value = match &self.magnitude {
            ModifierMagnitude::Flat(value) => *value,
            ModifierMagnitude::Calculated(calculation) => calculation.calculate(context),
        };

        ModifierSpec::new(self.id, self.op, final_value)
    }
}
