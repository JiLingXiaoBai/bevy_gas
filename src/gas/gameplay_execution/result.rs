use super::GameplayExecutionRequestId;
use crate::ability_system::AbilityActivationError;
use crate::gameplay_effects::GameplayEffectApplicationError;
use bevy::prelude::{Entity, Message};
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

/// Outcome of a queued request, preserving the concrete error when it failed.
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayExecutionOutcome {
    /// The request's primary operation succeeded. Follow-up requests have their own results.
    Succeeded,
    /// A gameplay rule rejected the request, such as cooldown, cost, or immunity.
    Rejected(GameplayExecutionError),
    /// Invalid configuration or missing runtime state prevented execution.
    Failed(GameplayExecutionError),
}

impl GameplayExecutionOutcome {
    pub(super) fn from_result(result: Result<(), GameplayExecutionError>) -> Self {
        match result {
            Ok(()) => Self::Succeeded,
            Err(error) if error.is_rejection() => Self::Rejected(error),
            Err(error) => Self::Failed(error),
        }
    }
}

/// Buffered result published by the global gameplay resolver in execution order.
///
/// Consume this message after `GameplayResolve`. Requests submitted by a result consumer run in
/// the next fixed tick because the current drain has finished. Synchronous activation uses a local
/// queue and does not publish results here. Success reports the primary request operation; it does
/// not turn ability activation effects or derived requests into a transaction.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct GameplayExecutionResult {
    /// Identifier returned when this request was accepted by the global queue.
    pub request_id: GameplayExecutionRequestId,
    /// Entity that submitted the activation or supplied the effect payload.
    pub source: Entity,
    /// Effect target, or the primary target captured by an ability activation.
    pub target: Entity,
    /// Success, gameplay rejection, or execution failure.
    pub outcome: GameplayExecutionOutcome,
}
