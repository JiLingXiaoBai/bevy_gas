use super::{ModifierEvaluationContext, ModifierSpec};
use crate::attributes::AttributeId;

/// Operation used to combine a modifier with an attribute value.
///
/// Discriminants match Unreal Engine's six gameplay modifier operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModifierOperation {
    /// Adds the evaluated magnitude before percentage, division, and multiplication.
    AddBase = 0,
    /// Combines multipliers as `1 + sum(magnitude - 1)`.
    ///
    /// A magnitude of `1.5` contributes a fifty-percent increase. Instant execution
    /// multiplies the base by the magnitude directly.
    MultiplyAdditive = 1,
    /// Combines divisors as `1 + sum(magnitude - 1)` before compound multiplication.
    ///
    /// A magnitude of `2.0` halves the value when used alone. Instant execution divides
    /// the base by the magnitude directly. The default executor and instant execution
    /// treat divisors with absolute value at most `1e-8` as one.
    DivideAdditive = 2,
    /// Replaces the evaluated value while the modifier is active.
    Override = 3,
    /// Multiplies the independently retained magnitudes together.
    ///
    /// Separate modifiers compound; effect stacking scales one magnitude by the stack
    /// count instead of raising it to that power.
    MultiplyCompound = 4,
    /// Adds the evaluated magnitude after percentage, division, and multiplication.
    ///
    /// Instant execution adds to the base immediately, in effect definition order.
    AddFinal = 5,
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
    /// Evaluates and returns the modifier magnitude without mutating gameplay state.
    ///
    /// Implementations choose an explicit fallback for unavailable context data. NaN and
    /// infinite results are rejected by effect application and attribute mutation APIs.
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
    ///
    /// This does not validate the result; effect application and attribute mutation APIs
    /// reject non-finite magnitudes before applying them.
    pub fn make_spec(&self, context: &dyn ModifierEvaluationContext) -> ModifierSpec {
        let final_value = match &self.magnitude {
            ModifierMagnitude::Flat(value) => *value,
            ModifierMagnitude::Calculated(calculation) => calculation.calculate(context),
        };

        ModifierSpec::new(self.id, self.op, final_value)
    }
}
