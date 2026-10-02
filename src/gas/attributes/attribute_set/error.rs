use super::super::{AttributeId, AttributeIdError};
use std::error::Error;
use std::fmt;

/// Describes why an operation on an [`AttributeSet`](super::AttributeSet) could not be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeSetError {
    /// The attribute ID is invalid for the supplied manager.
    AttributeId(AttributeIdError),
    /// The attribute ID is valid, but this set has not initialized its slot.
    UninitializedAttribute { id: AttributeId },
}

impl fmt::Display for AttributeSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttributeId(error) => write!(f, "attribute lookup failed: {error}"),
            Self::UninitializedAttribute { id } => write!(
                f,
                "attribute {} is not initialized in this AttributeSet",
                id.to_index()
            ),
        }
    }
}

impl Error for AttributeSetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AttributeId(error) => Some(error),
            Self::UninitializedAttribute { .. } => None,
        }
    }
}

impl From<AttributeIdError> for AttributeSetError {
    fn from(value: AttributeIdError) -> Self {
        Self::AttributeId(value)
    }
}
