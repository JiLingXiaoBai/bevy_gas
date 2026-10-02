//! Errors for targeting definitions, target acquisition, and activation targets.

use bevy::prelude::Entity;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

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

/// Describes why a targeting request could not produce target data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetingError {
    InvalidDefinition,
    InvalidOrigin,
    InvalidDirection,
    MissingSource { source: Entity },
    MissingExplicitTarget,
    TargetNotFound { target: Entity },
    TargetNotTargetable { target: Entity },
    NoTargetsFound,
}

impl fmt::Display for TargetingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition => write!(f, "targeting definition is not valid"),
            Self::InvalidOrigin => write!(f, "targeting origin must contain finite coordinates"),
            Self::InvalidDirection => write!(f, "targeting direction must be finite and non-zero"),
            Self::MissingSource { source } => {
                write!(
                    f,
                    "targeting source {source:?} does not have a GlobalTransform"
                )
            }
            Self::MissingExplicitTarget => {
                write!(f, "explicit target selection requires an explicit entity")
            }
            Self::TargetNotFound { target } => {
                write!(
                    f,
                    "target entity {target:?} does not have a GlobalTransform"
                )
            }
            Self::TargetNotTargetable { target } => {
                write!(f, "target entity {target:?} is not marked Targetable")
            }
            Self::NoTargetsFound => write!(f, "targeting request found no valid targets"),
        }
    }
}

impl Error for TargetingError {}
