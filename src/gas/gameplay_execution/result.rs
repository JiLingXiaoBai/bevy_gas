use super::{GameplayExecutionError, GameplayExecutionRequestId};
use bevy::prelude::{Entity, Message};

/// Outcome of a queued request, preserving the concrete error when it failed.
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayExecutionOutcome {
    /// The primary operation succeeded. Best-effort startup actions may still fail independently.
    /// Only follow-up actions explicitly submitted to the queue have their own results.
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
/// the next fixed tick because the current drain has finished. The synchronous activation API and
/// startup Instant actions do not publish results here. Success reports the primary request
/// operation; startup actions are best-effort, without transaction rollback.
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
