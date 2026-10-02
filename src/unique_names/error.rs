use core::fmt;
use std::error::Error;

/// Error returned when a unique name cannot be interned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueNameError {
    /// The `u32` handle space is exhausted.
    CapacityExceeded { max: u64 },
}

impl fmt::Display for UniqueNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded { max } => {
                write!(f, "unique name capacity exceeded; max names: {max}")
            }
        }
    }
}

impl Error for UniqueNameError {}
