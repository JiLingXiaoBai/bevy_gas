use super::{AbilitySystemParams, try_activate_ability_by_handle};
use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityChainContext, AbilityChainError, AbilitySpecHandle,
};
use crate::settings::GameplayAbilitySystemSettings;
use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct AbilityActivationRequest {
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    context: AbilityActivationContext,
}

impl AbilityActivationRequest {
    pub fn new(
        source: Entity,
        target: Entity,
        handle: AbilitySpecHandle,
        context: AbilityActivationContext,
    ) -> Self {
        Self {
            source,
            target,
            handle,
            context,
        }
    }

    pub fn get_source(&self) -> Entity {
        self.source
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_handle(&self) -> AbilitySpecHandle {
        self.handle
    }

    pub fn get_context(&self) -> &AbilityActivationContext {
        &self.context
    }
}

#[derive(Resource)]
pub struct AbilityActivationQueue {
    requests: VecDeque<AbilityActivationRequest>,
    max_activations_per_tick: usize,
    next_chain_id: u64,
}

impl Default for AbilityActivationQueue {
    fn default() -> Self {
        Self {
            requests: VecDeque::new(),
            max_activations_per_tick:
                GameplayAbilitySystemSettings::ABILITY_ACTIVATION_QUEUE_MAX_PER_TICK,
            next_chain_id: 1,
        }
    }
}

impl AbilityActivationQueue {
    pub fn push(&mut self, request: AbilityActivationRequest) {
        self.requests.push_back(request);
    }

    pub fn push_activation(
        &mut self,
        source: Entity,
        target: Entity,
        handle: AbilitySpecHandle,
        context: AbilityActivationContext,
    ) {
        self.push(AbilityActivationRequest::new(
            source, target, handle, context,
        ));
    }

    pub fn push_chained_activation(
        &mut self,
        source: Entity,
        target: Entity,
        handle: AbilitySpecHandle,
        parent_ability: Entity,
        parent_context: &AbilityActivationContext,
    ) -> Result<(), AbilityChainError> {
        let context = parent_context.child_for_chained_ability(parent_ability, handle)?;
        self.push(AbilityActivationRequest::new(
            source, target, handle, context,
        ));
        Ok(())
    }

    pub fn new_root_chain(&mut self, handle: AbilitySpecHandle) -> AbilityChainContext {
        let chain_id = self.next_chain_id;
        self.next_chain_id = self.next_chain_id.wrapping_add(1).max(1);
        AbilityChainContext::root(handle, chain_id)
    }

    pub fn pop(&mut self) -> Option<AbilityActivationRequest> {
        self.requests.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn clear(&mut self) {
        self.requests.clear();
    }

    pub fn max_activations_per_tick(&self) -> usize {
        self.max_activations_per_tick
    }

    pub fn set_max_activations_per_tick(&mut self, max_activations_per_tick: usize) {
        self.max_activations_per_tick = max_activations_per_tick.max(1);
    }
}

pub fn process_ability_activation_queue_system(
    mut activation_queue: ResMut<AbilityActivationQueue>,
    mut params: AbilitySystemParams,
) {
    let max_activations = activation_queue.max_activations_per_tick();
    for _ in 0..max_activations {
        let Some(request) = activation_queue.pop() else {
            return;
        };

        if let Err(error) = try_activate_ability_by_handle(
            request.get_source(),
            request.get_target(),
            request.get_handle(),
            request.get_context().clone(),
            &mut params,
        ) {
            if error.is_rejection() {
                debug!("queued ability activation was rejected: {error}");
            } else {
                error!("queued ability activation failed: {error}");
            }
        }
    }
}

pub fn ability_activation_queue_has_work(queue: Option<Res<AbilityActivationQueue>>) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}
