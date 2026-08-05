//! Shared attribute-modifier definitions, evaluation context, and evaluated values.
//!
//! This module deliberately does not depend on gameplay-effect runtime types.
//! Effects describe when a modifier is active, while attributes consume the
//! evaluated [`ModifierSpec`] using an opaque [`ModifierSourceId`].

mod context;
mod definition;
mod spec;

pub use context::ModifierEvaluationContext;
pub use definition::{
    Modifier, ModifierMagnitude, ModifierMagnitudeCalculation, ModifierOperation,
};
pub use spec::{AppliedModifier, ModifierSourceId, ModifierSpec};
