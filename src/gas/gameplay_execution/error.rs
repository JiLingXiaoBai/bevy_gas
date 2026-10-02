use crate::ability_system::AbilityActivationError;
use crate::gameplay_abilities::AbilityChainError;
use crate::gameplay_effects::GameplayEffectApplicationError;
use std::error::Error;
use std::fmt;

/// An error returned by the primary operation of one queued gameplay request.
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayExecutionError {
    /// Ability activation could not complete.
    AbilityActivation(AbilityActivationError),
    /// Gameplay-effect application could not complete.
    EffectApplication(GameplayEffectApplicationError),
}

impl GameplayExecutionError {
    /// Returns whether the failure is an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        match self {
            Self::AbilityActivation(error) => error.is_rejection(),
            Self::EffectApplication(error) => error.is_rejection(),
        }
    }
}

impl fmt::Display for GameplayExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbilityActivation(error) => fmt::Display::fmt(error, f),
            Self::EffectApplication(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for GameplayExecutionError {}

/// Describes why a gameplay request could not be added to the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameplayExecutionQueueError {
    /// The queue exhausted its monotonically increasing request identifiers.
    RequestIdExhausted,
    /// A chained activation would violate chain depth or repetition rules.
    InvalidChain(AbilityChainError),
}

impl fmt::Display for GameplayExecutionQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestIdExhausted => write!(f, "gameplay request identifiers are exhausted"),
            Self::InvalidChain(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for GameplayExecutionQueueError {}
