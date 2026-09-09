//! Per-actor bindings from game-defined input actions or slots to granted abilities.
//!
//! Device input, pressed state, and input buffering belong to the application's input layer.
//! Resolve a binding there and submit activation through the gameplay execution queue.

mod bindings;

pub use bindings::{AbilityInputBindingError, AbilityInputBindings};
