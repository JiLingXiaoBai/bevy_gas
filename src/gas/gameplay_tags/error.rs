use crate::unique_names::UniqueNameError;
use std::error::Error;
use std::fmt;

/// Describes why gameplay-tag registration, lookup, or reference-count mutation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayTagError {
    /// The tag name could not be interned.
    UniqueName(UniqueNameError),
    /// The configured gameplay-tag capacity was exceeded.
    CapacityExceeded { max: usize },
    /// A tag or one of its ancestors would exceed its per-entity reference capacity.
    ReferenceCountOverflow {
        /// Index of the tag whose total reference count would overflow.
        index: usize,
        /// Maximum total reference count for that tag.
        max: u16,
    },
    /// A tag index was outside the registered range.
    InvalidTagIndex { index: usize },
}

impl fmt::Display for GameplayTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UniqueName(error) => write!(f, "gameplay tag registration failed: {error}"),
            Self::CapacityExceeded { max } => {
                write!(f, "gameplay tag capacity exceeded; max tags: {max}")
            }
            Self::ReferenceCountOverflow { index, max } => {
                write!(
                    f,
                    "gameplay tag {index} reference count exceeded; max references: {max}"
                )
            }
            Self::InvalidTagIndex { index } => {
                write!(f, "invalid gameplay tag index: {index}")
            }
        }
    }
}

impl Error for GameplayTagError {}

impl From<UniqueNameError> for GameplayTagError {
    fn from(value: UniqueNameError) -> Self {
        Self::UniqueName(value)
    }
}
