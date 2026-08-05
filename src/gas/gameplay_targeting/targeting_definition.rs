use crate::gameplay_tags::TagRequirements;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;

/// Marks an entity as eligible for non-self targeting selection.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Targetable;

/// Controls distance sorting for targeting candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetingSortOrder {
    Ascending,
    Descending,
}

/// One ordered operation in a targeting definition.
#[derive(Clone)]
pub enum TargetingOperation {
    /// Selects the request source. The source does not need [`Targetable`].
    SelectSelf,
    /// Selects the explicit entity supplied by the request.
    SelectExplicitEntity,
    /// Selects targetable entities inside a world-space sphere.
    SelectSphere { radius: f32 },
    /// Selects targetable entities inside a world-space cone.
    SelectCone {
        radius: f32,
        half_angle_radians: f32,
    },
    /// Removes the request source from the candidates.
    FilterSource,
    /// Retains candidates whose gameplay tags pass the requirements.
    FilterTags { requirements: TagRequirements },
    /// Retains candidates that have an [`crate::AttributeSet`] component.
    RequireAttributeSet,
    /// Retains candidates no farther than `max_distance` from the request origin.
    FilterDistance { max_distance: f32 },
    /// Sorts candidates by squared distance from the request origin.
    SortByDistance { order: TargetingSortOrder },
    /// Retains at most the first `count` candidates.
    Limit { count: usize },
}

impl TargetingOperation {
    pub(crate) fn is_selection(&self) -> bool {
        matches!(
            self,
            Self::SelectSelf
                | Self::SelectExplicitEntity
                | Self::SelectSphere { .. }
                | Self::SelectCone { .. }
        )
    }
}

/// Describes an invalid targeting operation pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetingDefinitionError {
    EmptyOperations,
    SelectionMustBeFirst,
    MultipleSelections,
    InvalidRadius { radius: f32 },
    InvalidDistance { max_distance: f32 },
    InvalidHalfAngle { half_angle_radians: f32 },
    ZeroLimit,
}

impl fmt::Display for TargetingDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOperations => write!(f, "targeting definition has no operations"),
            Self::SelectionMustBeFirst => {
                write!(f, "the first targeting operation must select candidates")
            }
            Self::MultipleSelections => {
                write!(
                    f,
                    "a targeting definition may contain only one selection operation"
                )
            }
            Self::InvalidRadius { radius } => {
                write!(
                    f,
                    "targeting radius must be finite and non-negative, got {radius}"
                )
            }
            Self::InvalidDistance { max_distance } => write!(
                f,
                "targeting distance must be finite and non-negative, got {max_distance}"
            ),
            Self::InvalidHalfAngle { half_angle_radians } => write!(
                f,
                "targeting cone half angle must be finite and in [0, PI], got {half_angle_radians}"
            ),
            Self::ZeroLimit => write!(f, "targeting result limit must be greater than zero"),
        }
    }
}

impl Error for TargetingDefinitionError {}

/// An immutable, validated sequence of target selection and refinement operations.
pub struct TargetingDefinition {
    operations: Vec<TargetingOperation>,
}

impl TargetingDefinition {
    /// Creates and validates an ordered targeting operation pipeline.
    ///
    /// The first operation must be exactly one selection operation. All later
    /// operations refine or order that candidate set.
    ///
    /// # Errors
    ///
    /// Returns [`TargetingDefinitionError`] when the operation list is empty,
    /// does not start with exactly one selection, or contains invalid numeric limits.
    pub fn new(operations: Vec<TargetingOperation>) -> Result<Self, TargetingDefinitionError> {
        let Some(first) = operations.first() else {
            return Err(TargetingDefinitionError::EmptyOperations);
        };
        if !first.is_selection() {
            return Err(TargetingDefinitionError::SelectionMustBeFirst);
        }
        if operations
            .iter()
            .skip(1)
            .any(TargetingOperation::is_selection)
        {
            return Err(TargetingDefinitionError::MultipleSelections);
        }

        for operation in &operations {
            match operation {
                TargetingOperation::SelectSphere { radius } => validate_radius(*radius)?,
                TargetingOperation::SelectCone {
                    radius,
                    half_angle_radians,
                } => {
                    validate_radius(*radius)?;
                    if !half_angle_radians.is_finite()
                        || !(0.0..=std::f32::consts::PI).contains(half_angle_radians)
                    {
                        return Err(TargetingDefinitionError::InvalidHalfAngle {
                            half_angle_radians: *half_angle_radians,
                        });
                    }
                }
                TargetingOperation::Limit { count: 0 } => {
                    return Err(TargetingDefinitionError::ZeroLimit);
                }
                TargetingOperation::FilterDistance { max_distance }
                    if !max_distance.is_finite() || *max_distance < 0.0 =>
                {
                    return Err(TargetingDefinitionError::InvalidDistance {
                        max_distance: *max_distance,
                    });
                }
                _ => {}
            }
        }

        Ok(Self { operations })
    }

    /// Returns the validated operations in execution order.
    pub fn get_operations(&self) -> &[TargetingOperation] {
        &self.operations
    }
}

fn validate_radius(radius: f32) -> Result<(), TargetingDefinitionError> {
    if !radius.is_finite() || radius < 0.0 {
        return Err(TargetingDefinitionError::InvalidRadius { radius });
    }
    Ok(())
}
