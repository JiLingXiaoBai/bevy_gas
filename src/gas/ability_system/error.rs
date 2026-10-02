//! Errors from granting ability specifications.

use std::error::Error;
use std::fmt;

/// Failure to grant an ability specification to an ability-system component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityGrantError {
    /// Every owner-local `u32` handle has already been allocated.
    HandleExhausted,
}

impl fmt::Display for AbilityGrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandleExhausted => write!(f, "ability specification handles are exhausted"),
        }
    }
}

impl Error for AbilityGrantError {}
