//! Ability activation validation, startup, and execution flow.

mod error;
mod execution;
mod startup;
mod validation;

pub use error::AbilityActivationError;
pub(crate) use execution::execute_ability_activation_in_batch;
pub use execution::try_activate_ability_by_handle;
pub use validation::can_activate_ability;
