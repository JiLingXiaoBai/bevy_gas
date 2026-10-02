//! Errors and conversions for gameplay-effect preparation and execution.

use super::state::ActiveEffectStorageError;
use crate::attributes::{AttributeId, AttributeIdError, AttributeSetError};
use crate::gameplay_tags::GameplayTagError;
use bevy::prelude::Entity;
use std::error::Error;
use std::fmt;

/// Describes why a gameplay effect could not be prepared or executed.
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayEffectApplicationError {
    /// The configured application probability is not finite or outside `0.0..=1.0`.
    InvalidProbability { probability: f32 },
    /// The probability roll rejected the application.
    ProbabilityRejected,
    /// Source or target application tag requirements were not met.
    ApplicationRequirementsNotMet,
    /// An active immunity effect blocked the application.
    BlockedByImmunity,
    /// A duration effect resolved to zero ticks.
    InvalidDuration,
    /// The target cannot store active gameplay effects.
    MissingActiveGameplayEffects { target: Entity },
    /// The target's container was installed without an available runtime storage identity.
    UninitializedActiveEffectStorage { target: Entity },
    /// The target has exhausted the representable active-effect slot space.
    ActiveEffectCapacityExceeded { target: Entity },
    /// The target does not have the attribute storage required by the effect.
    MissingAttributeSet { target: Entity },
    /// The target has attribute storage but has not initialized a modified attribute.
    MissingAttribute { target: Entity, id: AttributeId },
    /// The target does not have the tag container required by the effect.
    MissingTagContainer { target: Entity },
    /// The stacking policy rejected an application beyond its limit.
    StackOverflowRejected,
    /// A gameplay tag did not belong to the active tag manager.
    GameplayTag(GameplayTagError),
    /// An attribute ID did not belong to the active attribute manager.
    AttributeId(AttributeIdError),
}

impl fmt::Display for GameplayEffectApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProbability { probability } => write!(
                f,
                "gameplay effect probability must be finite and within 0.0..=1.0, got {probability}"
            ),
            Self::ProbabilityRejected => {
                write!(f, "gameplay effect probability roll rejected application")
            }
            Self::ApplicationRequirementsNotMet => write!(
                f,
                "gameplay effect application tag requirements were not met"
            ),
            Self::BlockedByImmunity => {
                write!(f, "gameplay effect application was blocked by immunity")
            }
            Self::InvalidDuration => {
                write!(
                    f,
                    "gameplay effect duration must be greater than zero ticks"
                )
            }
            Self::MissingActiveGameplayEffects { target } => write!(
                f,
                "gameplay effect target {target:?} has no ActiveGameplayEffects"
            ),
            Self::UninitializedActiveEffectStorage { target } => write!(
                f,
                "gameplay effect target {target:?} has no initialized effect storage identity; install the GAS runtime before its component"
            ),
            Self::ActiveEffectCapacityExceeded { target } => write!(
                f,
                "gameplay effect target {target:?} exhausted active-effect handle capacity"
            ),
            Self::MissingAttributeSet { target } => {
                write!(f, "gameplay effect target {target:?} has no AttributeSet")
            }
            Self::MissingAttribute { target, id } => write!(
                f,
                "gameplay effect target {target:?} has not initialized attribute {}",
                id.to_index()
            ),
            Self::MissingTagContainer { target } => write!(
                f,
                "gameplay effect target {target:?} has no GameplayTagContainer"
            ),
            Self::StackOverflowRejected => {
                write!(f, "gameplay effect stacking policy rejected overflow")
            }
            Self::GameplayTag(error) => {
                write!(f, "gameplay effect contains an invalid tag: {error}")
            }
            Self::AttributeId(error) => write!(
                f,
                "gameplay effect contains an invalid attribute ID: {error}"
            ),
        }
    }
}

impl Error for GameplayEffectApplicationError {}

impl GameplayEffectApplicationError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub const fn is_rejection(&self) -> bool {
        matches!(
            self,
            Self::ProbabilityRejected
                | Self::ApplicationRequirementsNotMet
                | Self::BlockedByImmunity
                | Self::StackOverflowRejected
        )
    }
}

impl From<GameplayTagError> for GameplayEffectApplicationError {
    fn from(value: GameplayTagError) -> Self {
        Self::GameplayTag(value)
    }
}

impl From<AttributeIdError> for GameplayEffectApplicationError {
    fn from(value: AttributeIdError) -> Self {
        Self::AttributeId(value)
    }
}

impl From<ActiveEffectStorageError> for GameplayEffectApplicationError {
    fn from(value: ActiveEffectStorageError) -> Self {
        match value {
            ActiveEffectStorageError::Uninitialized { target } => {
                Self::UninitializedActiveEffectStorage { target }
            }
            ActiveEffectStorageError::CapacityExceeded { target } => {
                Self::ActiveEffectCapacityExceeded { target }
            }
        }
    }
}

pub(super) fn map_attribute_set_error(
    target: Entity,
    error: AttributeSetError,
) -> GameplayEffectApplicationError {
    match error {
        AttributeSetError::AttributeId(error) => GameplayEffectApplicationError::AttributeId(error),
        AttributeSetError::UninitializedAttribute { id } => {
            GameplayEffectApplicationError::MissingAttribute { target, id }
        }
    }
}
