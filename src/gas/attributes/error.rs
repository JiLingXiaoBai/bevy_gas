use super::{AttributeId, AttributeRegion};
use crate::UniqueNameError;
use std::error::Error;
use std::fmt;

/// Describes why attribute registration or location lookup failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeIdError {
    /// The attribute name could not be interned.
    UniqueName(UniqueNameError),
    /// The combined hot and cold attribute capacity was exceeded.
    CapacityExceeded { max: usize },
    /// One storage region reached its configured capacity.
    RegionCapacityExceeded { region: AttributeRegion, max: usize },
    /// An existing attribute was requested with a different region.
    RegionMismatch {
        existing: AttributeRegion,
        requested: AttributeRegion,
    },
    /// Internal registration data was inconsistent for an existing ID.
    MissingLocation { id: AttributeId },
}

impl fmt::Display for AttributeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeIdError::UniqueName(err) => {
                write!(f, "attribute ID registration failed: {err}")
            }
            AttributeIdError::CapacityExceeded { max } => {
                write!(f, "attribute id capacity exceeded; max attributes: {max}")
            }
            AttributeIdError::RegionCapacityExceeded { region, max } => {
                write!(
                    f,
                    "{region:?} attribute capacity exceeded; max attributes in region: {max}"
                )
            }
            AttributeIdError::RegionMismatch {
                existing,
                requested,
            } => write!(
                f,
                "attribute was already registered as {existing:?}, but {requested:?} was requested"
            ),
            AttributeIdError::MissingLocation { id } => write!(
                f,
                "attribute ID {} is missing its storage location",
                id.to_index()
            ),
        }
    }
}

impl Error for AttributeIdError {}

impl From<UniqueNameError> for AttributeIdError {
    fn from(value: UniqueNameError) -> Self {
        Self::UniqueName(value)
    }
}
