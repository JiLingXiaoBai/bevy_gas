use super::{AbilityActivationRequest, GameplayEffectApplicationRequest, GameplayExecutionRequest};
use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityChainContext, AbilityChainError, AbilitySpecHandle,
    ActiveAbilityHandle,
};
use crate::gameplay_effects::{EffectPayload, GameplayEffect};
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Queue-local identifier for one accepted gameplay request.
///
/// IDs increase in submission order and are not reused by `clear`. Only IDs from the global
/// resource correspond to [`super::GameplayExecutionResult`] messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplayExecutionRequestId(u64);

impl GameplayExecutionRequestId {
    /// Returns the numeric identifier within the queue that accepted the request.
    pub fn get_value(self) -> u64 {
        self.0
    }
}

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

/// Global FIFO for gameplay mutations consumed during
/// [`GameplayAbilitySystemSet::GameplayResolve`](crate::GameplayAbilitySystemSet::GameplayResolve).
///
/// All request-producing systems that require same-tick execution must run before the resolver.
/// Requests appended while the resolver is running are consumed by the same drain operation.
#[derive(Resource)]
pub struct GameplayExecutionQueue {
    requests: VecDeque<(GameplayExecutionRequestId, GameplayExecutionRequest)>,
    next_request_id: u64,
    next_chain_id: u64,
}

impl Default for GameplayExecutionQueue {
    fn default() -> Self {
        Self {
            requests: VecDeque::new(),
            next_request_id: 1,
            next_chain_id: 1,
        }
    }
}

impl GameplayExecutionQueue {
    /// Appends `request` and returns the identifier used by its execution result.
    ///
    /// Returns [`GameplayExecutionQueueError::RequestIdExhausted`] without enqueuing on exhaustion.
    pub fn push(
        &mut self,
        request: impl Into<GameplayExecutionRequest>,
    ) -> Result<GameplayExecutionRequestId, GameplayExecutionQueueError> {
        let next_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(GameplayExecutionQueueError::RequestIdExhausted)?;
        let id = GameplayExecutionRequestId(self.next_request_id);
        self.requests.push_back((id, request.into()));
        self.next_request_id = next_id;
        Ok(id)
    }

    /// Appends an ability activation for `handle` from `source` against `targets`, preserving
    /// the supplied activation `context`.
    ///
    /// Returns the accepted request ID, or an error if identifiers are exhausted.
    pub fn push_activation(
        &mut self,
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        handle: AbilitySpecHandle,
        context: AbilityActivationContext,
    ) -> Result<GameplayExecutionRequestId, GameplayExecutionQueueError> {
        self.push(AbilityActivationRequest::new(
            source, targets, handle, context,
        ))
    }

    /// Appends a chained activation for `handle` derived from `parent_context` and
    /// `parent_ability`.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayExecutionQueueError`] for an invalid chain or exhausted request IDs.
    pub fn push_chained_activation(
        &mut self,
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        handle: AbilitySpecHandle,
        parent_ability: ActiveAbilityHandle,
        parent_context: &AbilityActivationContext,
    ) -> Result<GameplayExecutionRequestId, GameplayExecutionQueueError> {
        let context = parent_context
            .child_for_chained_ability(parent_ability, handle)
            .map_err(GameplayExecutionQueueError::InvalidChain)?;
        self.push(AbilityActivationRequest::new(
            source, targets, handle, context,
        ))
    }

    /// Creates a root chain context for `handle` with a queue-local identifier.
    ///
    /// Returns the new root context. Queue-local identifiers are deterministic for enqueue order.
    pub fn new_root_chain(&mut self, handle: AbilitySpecHandle) -> AbilityChainContext {
        let chain_id = self.next_chain_id;
        self.next_chain_id = self.next_chain_id.wrapping_add(1).max(1);
        AbilityChainContext::root(handle, chain_id)
    }

    /// Appends an application of `effect` to `target` with the captured `payload`.
    ///
    /// Returns the accepted request ID, or an error if identifiers are exhausted.
    pub fn push_application(
        &mut self,
        target: Entity,
        effect: Arc<GameplayEffect>,
        payload: EffectPayload,
    ) -> Result<GameplayExecutionRequestId, GameplayExecutionQueueError> {
        self.push(GameplayEffectApplicationRequest::new(
            target, effect, payload,
        ))
    }

    /// Removes and returns the oldest request with its identifier, or `None` when empty.
    pub fn pop(&mut self) -> Option<(GameplayExecutionRequestId, GameplayExecutionRequest)> {
        self.requests.pop_front()
    }

    /// Returns whether the queue has no pending requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Returns the number of pending requests.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Removes all pending requests without executing them or publishing execution results.
    /// Request identifiers remain reserved and will not be reused.
    pub fn clear(&mut self) {
        self.requests.clear();
    }
}
