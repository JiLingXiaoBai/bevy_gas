use super::ModifierOperation;
use crate::attributes::AttributeId;

/// Opaque, generational identity of the runtime source owning a modifier.
///
/// `scope` identifies the owning storage, while `slot` and `generation`
/// distinguish entries and prevent stale sources from removing newer values.
/// The type is shared and does not depend on any particular effect container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierSourceId {
    scope: u64,
    slot: u32,
    generation: u32,
}

impl ModifierSourceId {
    /// Creates a source ID from an owning scope, slot, and generation.
    pub const fn new(scope: u64, slot: u32, generation: u32) -> Self {
        Self {
            scope,
            slot,
            generation,
        }
    }

    /// Returns the caller-defined owning scope.
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
    /// Creates an evaluated modifier specification.
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

    /// Returns a copy whose magnitude is multiplied by `stack_count`.
    pub fn scaled_by_stack(&self, stack_count: u32) -> Self {
        Self {
            id: self.id,
            op: self.op,
            value: self.value * stack_count as f32,
        }
    }
}

/// Evaluated value retained by an attribute aggregator.
///
/// This public type is retained for API compatibility. Attribute aggregation
/// treats it as an implementation detail and identifies ownership through the
/// neutral [`ModifierSourceId`].
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
