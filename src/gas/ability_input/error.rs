use crate::gameplay_abilities::AbilitySpecHandle;
use std::error::Error;
use std::fmt;

/// Explains why a logical input could not resolve to a currently granted ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityInputBindingError {
    /// The requested action has no binding.
    UnboundInput,
    /// The action is bound, but its handle is absent from the supplied ASC.
    AbilityNotGranted {
        /// The stale or invalid handle stored in the binding.
        handle: AbilitySpecHandle,
    },
}

impl fmt::Display for AbilityInputBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundInput => formatter.write_str("the input action is not bound"),
            Self::AbilityNotGranted { handle } => write!(
                formatter,
                "the bound ability handle {} is not granted by this ASC",
                handle.get_value()
            ),
        }
    }
}

impl Error for AbilityInputBindingError {}
