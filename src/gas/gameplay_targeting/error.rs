//! Errors for constructing captured ability activation targets.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// An error returned when constructing validated ability activation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationTargetsError {
    /// Acquired target data did not contain a primary target.
    EmptyTargetData,
}

impl Display for AbilityActivationTargetsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTargetData => {
                formatter.write_str("ability activation target data must not be empty")
            }
        }
    }
}

impl Error for AbilityActivationTargetsError {}
