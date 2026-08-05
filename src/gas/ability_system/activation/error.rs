use super::super::commit::AbilityCommitError;
use crate::gameplay_abilities::{AbilityChainError, AbilitySpecHandle};
use crate::gameplay_tags::GameplayTagError;
use bevy::prelude::Entity;
use std::error::Error;
use std::fmt;

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
            Self::ActivationRequirementsNotMet { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} does not meet activation requirements",
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

impl Error for AbilityActivationError {}

impl AbilityActivationError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        match self {
            Self::MultipleInstancesNotAllowed { .. }
            | Self::ActivationRequirementsNotMet { .. } => true,
            Self::CommitPreparationFailed { error, .. } => error.is_rejection(),
            Self::InvalidChain(_)
            | Self::MissingAbilitySystemComponent { .. }
            | Self::AbilityNotFound { .. }
            | Self::StartFailed { .. }
            | Self::CancellationFailed { .. }
            | Self::CommitExecutionFailed { .. } => false,
        }
    }
}

pub(super) fn ability_activation_failed(
    error: AbilityActivationError,
) -> Result<(), AbilityActivationError> {
    Err(error)
}
