use super::ModifierOperation;
use crate::attributes::AttributeId;

/// Opaque, generational identity of the runtime source owning a modifier.
///
/// `scope` identifies the owning storage, while `slot` and `generation`
/// distinguish entries and prevent stale sources from removing newer values.
/// Caller-defined and runtime-allocated sources occupy separate identity domains,
/// even when all three numeric fields match. This type is independent of effect storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierSourceId {
    domain: ModifierSourceDomain,
    scope: u64,
    slot: u32,
    generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ModifierSourceDomain {
    Caller,
    Runtime,
}

impl ModifierSourceId {
    /// Creates a caller-defined source ID from an owning scope, slot, and generation.
    ///
    /// Caller-defined IDs cannot collide with runtime-allocated IDs. Callers remain
    /// responsible for coordinating numeric identities within the caller domain.
    pub const fn new(scope: u64, slot: u32, generation: u32) -> Self {
        Self {
            domain: ModifierSourceDomain::Caller,
            scope,
            slot,
            generation,
        }
    }

    pub(crate) const fn new_runtime(scope: u64, slot: u32, generation: u32) -> Self {
        Self {
            domain: ModifierSourceDomain::Runtime,
            scope,
            slot,
            generation,
        }
    }

    /// Returns the owning scope within this source's identity domain.
    pub const fn get_scope(self) -> u64 {
        self.scope
    }

    /// Returns the source slot within its owning storage.
    pub const fn get_slot(self) -> u32 {
        self.slot
    }

    /// Returns the source generation.
    pub const fn get_generation(self) -> u32 {
        self.generation
    }
}

/// Evaluated modifier value ready to be applied to an attribute.
#[derive(Debug, Clone, Copy)]
pub struct ModifierSpec {
    id: AttributeId,
    op: ModifierOperation,
    value: f32,
}

impl ModifierSpec {
    /// Creates an evaluated modifier specification without validating `value`.
    ///
    /// Effect application and attribute mutation APIs reject non-finite magnitudes.
    pub const fn new(id: AttributeId, op: ModifierOperation, value: f32) -> Self {
        Self { id, op, value }
    }

    /// Returns the target attribute ID.
    pub const fn get_id(&self) -> AttributeId {
        self.id
    }

    /// Returns the operation applied to the target attribute.
    pub const fn get_operation(&self) -> ModifierOperation {
        self.op
    }

    /// Returns the evaluated magnitude.
    pub const fn get_value(&self) -> f32 {
        self.value
    }

    /// Returns a copy with the magnitude scaled for `stack_count` and its operation.
    ///
    /// Additive multipliers and divisors use `1 + (value - 1) * stack_count`;
    /// overrides retain their value. Base addition, compound multiplication, and final
    /// addition use `value * stack_count`. Thus separate compound modifiers multiply,
    /// but repeated stacks of one effect scale its magnitude linearly, as in Unreal.
    /// A zero count produces the operation's bias (zero or one), except for overrides.
    /// The result can overflow; effect application validates it before mutation.
    pub fn scaled_by_stack(&self, stack_count: u32) -> Self {
        let value = match self.op {
            ModifierOperation::Override => self.value,
            operation => {
                let bias = match operation {
                    ModifierOperation::MultiplyAdditive | ModifierOperation::DivideAdditive => 1.0,
                    _ => 0.0,
                };
                (self.value - bias) * stack_count as f32 + bias
            }
        };
        Self {
            id: self.id,
            op: self.op,
            value,
        }
    }
}

/// Evaluated value retained by an attribute aggregator.
///
/// Custom executors borrow these values through
/// [`Aggregator::modifiers`](crate::attributes::Aggregator::modifiers).
/// Ownership is identified through the neutral [`ModifierSourceId`].
#[derive(Debug, Clone, Copy)]
pub struct AppliedModifier {
    source_id: ModifierSourceId,
    value: f32,
}

impl AppliedModifier {
    /// Creates an applied modifier owned by `source_id`.
    pub const fn new(source_id: ModifierSourceId, value: f32) -> Self {
        Self { source_id, value }
    }

    /// Returns the runtime source that owns this value.
    pub const fn get_source_id(&self) -> ModifierSourceId {
        self.source_id
    }

    /// Returns the runtime source that owns this value.
    ///
    /// Prefer [`Self::get_source_id`] in new code. This name is retained to
    /// minimize churn for code that previously inspected applied modifiers.
    pub const fn get_handle(&self) -> ModifierSourceId {
        self.source_id
    }

    /// Returns the evaluated magnitude.
    pub const fn get_value(&self) -> f32 {
        self.value
    }
}
