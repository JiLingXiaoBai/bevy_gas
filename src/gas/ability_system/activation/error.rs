use super::super::commit::AbilityCommitError;
use crate::gameplay_abilities::{AbilityChainError, AbilitySpecHandle};
use crate::gameplay_tags::GameplayTagError;
use bevy::prelude::Entity;
use std::error::Error;
use std::fmt;

/// A tag or cooldown requirement shared by activation prechecks and actual activation.
///
/// These failures are gameplay rejections. Cost validation and commit failures use
/// [`AbilityCommitError`] separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationRequirementError {
    /// Another active ability blocks this definition's asset tags.
    BlockedByAbility,
    /// The owner has a tag that blocks activation.
    ActivationBlocked,
    /// The owner's tag container does not satisfy the required activation tags.
    MissingRequiredTags,
    /// The owner currently has a granted cooldown tag.
    CooldownActive,
}

impl fmt::Display for AbilityActivationRequirementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockedByAbility => write!(f, "another active ability blocks activation"),
            Self::ActivationBlocked => write!(f, "an owned gameplay tag blocks activation"),
            Self::MissingRequiredTags => write!(f, "required activation tags are missing"),
            Self::CooldownActive => write!(f, "the ability is on cooldown"),
        }
    }
}

impl Error for AbilityActivationRequirementError {}

/// Reason a read-only activation precheck found an ability unavailable.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityActivationCheckError {
    /// A tag or cooldown requirement rejected activation.
    Requirements(AbilityActivationRequirementError),
    /// Cost evaluation or affordability checking failed.
    Cost(AbilityCommitError),
}

impl fmt::Display for AbilityActivationCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requirements(error) => fmt::Display::fmt(error, f),
            Self::Cost(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for AbilityActivationCheckError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Requirements(error) => Some(error),
            Self::Cost(error) => Some(error),
        }
    }
}

/// Describes why an ability could not be activated.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityActivationError {
    /// The supplied ability-chain context is invalid.
    InvalidChain(AbilityChainError),
    /// The source entity has no ability-system component.
    MissingAbilitySystemComponent { source: Entity },
    /// The source does not own the requested ability handle.
    AbilityNotFound {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    /// The ability disallows another simultaneous instance.
    MultipleInstancesNotAllowed {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    /// Gameplay-tag or cooldown requirements rejected activation.
    ActivationRequirementsNotMet {
        source: Entity,
        handle: AbilitySpecHandle,
        /// The concrete tag or cooldown requirement that rejected activation.
        error: AbilityActivationRequirementError,
    },
    /// Cost or cooldown preparation failed.
    CommitPreparationFailed {
        source: Entity,
        handle: AbilitySpecHandle,
        error: AbilityCommitError,
    },
    /// Runtime state for the active ability could not be created.
    StartFailed {
        source: Entity,
        handle: AbilitySpecHandle,
        error: GameplayTagError,
    },
    /// An ability selected for cancellation could not be cancelled.
    CancellationFailed {
        source: Entity,
        handle: AbilitySpecHandle,
        error: GameplayTagError,
    },
    /// A prepared cost or cooldown could not be executed.
    CommitExecutionFailed {
        source: Entity,
        handle: AbilitySpecHandle,
        error: AbilityCommitError,
    },
}

impl fmt::Display for AbilityActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChain(err) => write!(f, "ability activation failed: {err}"),
            Self::MissingAbilitySystemComponent { source } => write!(
                f,
                "ability activation failed: source entity {source:?} has no AbilitySystemComponent"
            ),
            Self::AbilityNotFound { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} has no ability handle {}",
                handle.get_value()
            ),
            Self::MultipleInstancesNotAllowed { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} is already active",
                handle.get_value()
            ),
            Self::ActivationRequirementsNotMet {
                source,
                handle,
                error,
            } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} does not meet activation requirements: {error}",
                handle.get_value()
            ),
            Self::CommitPreparationFailed {
                source,
                handle,
                error,
            } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not prepare cost or cooldown: {error}",
                handle.get_value(),
            ),
            Self::StartFailed {
                source,
                handle,
                error,
            } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not start: {error}",
                handle.get_value(),
            ),
            Self::CancellationFailed {
                source,
                handle,
                error,
            } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not cancel matching abilities: {error}",
                handle.get_value(),
            ),
            Self::CommitExecutionFailed {
                source,
                handle,
                error,
            } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not execute cost or cooldown: {error}",
                handle.get_value(),
            ),
        }
    }
}

impl Error for AbilityActivationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidChain(error) => Some(error),
            Self::ActivationRequirementsNotMet { error, .. } => Some(error),
            Self::CommitPreparationFailed { error, .. }
            | Self::CommitExecutionFailed { error, .. } => Some(error),
            Self::StartFailed { error, .. } | Self::CancellationFailed { error, .. } => Some(error),
            Self::MissingAbilitySystemComponent { .. }
            | Self::AbilityNotFound { .. }
            | Self::MultipleInstancesNotAllowed { .. } => None,
        }
    }
}

impl AbilityActivationError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        match self {
            Self::MultipleInstancesNotAllowed { .. }
            | Self::ActivationRequirementsNotMet { .. } => true,
            Self::CommitPreparationFailed { error, .. } => error.is_rejection(),
            Self::CommitExecutionFailed {
                error: AbilityCommitError::AdditionalCost(error),
                ..
            } => error.is_rejection(),
            Self::InvalidChain(_)
            | Self::MissingAbilitySystemComponent { .. }
            | Self::AbilityNotFound { .. }
            | Self::StartFailed { .. }
            | Self::CancellationFailed { .. }
            | Self::CommitExecutionFailed { .. } => false,
        }
    }
}

pub(super) fn ability_activation_failed<T>(
    error: AbilityActivationError,
) -> Result<T, AbilityActivationError> {
    Err(error)
}
