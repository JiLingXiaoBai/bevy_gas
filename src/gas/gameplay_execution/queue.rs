use super::{AbilityActivationRequest, GameplayEffectApplicationRequest, GameplayExecutionRequest};
use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityChainContext, AbilityChainError, AbilitySpecHandle,
    ActiveAbilityHandle,
};
use crate::gameplay_effects::{EffectPayload, GameplayEffect};
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;

/// Global FIFO for gameplay mutations consumed during
/// [`GameplayAbilitySystemSet::GameplayResolve`](crate::GameplayAbilitySystemSet::GameplayResolve).
///
/// All request-producing systems that require same-tick execution must run before the resolver.
/// Requests appended while the resolver is running are consumed by the same drain operation.
#[derive(Resource)]
pub struct GameplayExecutionQueue {
    requests: VecDeque<GameplayExecutionRequest>,
    next_chain_id: u64,
}

impl Default for GameplayExecutionQueue {
    fn default() -> Self {
        Self {
            requests: VecDeque::new(),
            next_chain_id: 1,
        }
    }
}

impl GameplayExecutionQueue {
    /// Appends `request` to the shared FIFO.
    pub fn push(&mut self, request: impl Into<GameplayExecutionRequest>) {
        self.requests.push_back(request.into());
    }

    /// Appends an ability activation for `handle` from `source` against `targets`, preserving
    /// the supplied activation `context`.
    pub fn push_activation(
        &mut self,
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        handle: AbilitySpecHandle,
        context: AbilityActivationContext,
    ) {
        self.push(AbilityActivationRequest::new(
            source, targets, handle, context,
        ));
    }

    /// Appends a chained activation for `handle` derived from `parent_context` and
    /// `parent_ability`.
    ///
    /// # Errors
    ///
    /// Returns [`AbilityChainError`] when the chain would exceed its depth or repeat an ability.
    pub fn push_chained_activation(
        &mut self,
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        handle: AbilitySpecHandle,
        parent_ability: ActiveAbilityHandle,
        parent_context: &AbilityActivationContext,
    ) -> Result<(), AbilityChainError> {
        let context = parent_context.child_for_chained_ability(parent_ability, handle)?;
        self.push(AbilityActivationRequest::new(
            source, targets, handle, context,
        ));
        Ok(())
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
    pub fn push_application(
        &mut self,
        target: Entity,
        effect: Arc<GameplayEffect>,
        payload: EffectPayload,
    ) {
        self.push(GameplayEffectApplicationRequest::new(
            target, effect, payload,
        ));
    }

    /// Removes and returns the oldest request, or `None` when the FIFO is empty.
    pub fn pop(&mut self) -> Option<GameplayExecutionRequest> {
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

    /// Removes all pending requests without executing them.
    pub fn clear(&mut self) {
        self.requests.clear();
    }
}
