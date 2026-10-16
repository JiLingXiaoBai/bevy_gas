use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Explains why an opted-in entity's logical position could not be presented.
///
/// Presentation keeps the existing transform and discards interpolation history on error.
/// Correct the entity's components or position to recover on the next synchronization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionPresentationError {
    /// The entity has no authoritative logical position.
    MissingLogicalPosition,
    /// The required rendering transform has been removed from the entity.
    MissingTransform,
    /// The logical position contains a non-finite coordinate.
    NonFiniteLogicalPosition,
    /// The entity has a parent, so its local translation cannot represent a world position.
    ParentedEntity,
}

impl Display for PositionPresentationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            Self::MissingLogicalPosition => "position presentation requires a LogicalPosition",
            Self::MissingTransform => "position presentation requires a Transform",
            Self::NonFiniteLogicalPosition => {
                "position presentation requires a finite logical position"
            }
            Self::ParentedEntity => "position presentation requires an entity without a parent",
        })
    }
}

impl Error for PositionPresentationError {}
