use super::{EffectPayload, GameplayEffect, apply_gameplay_effect};
use crate::ability_system::AbilitySystemParams;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone)]
pub struct GameplayEffectApplicationRequest {
    target: Entity,
    effect: Arc<GameplayEffect>,
    payload: EffectPayload,
}

impl GameplayEffectApplicationRequest {
    pub fn new(target: Entity, effect: Arc<GameplayEffect>, payload: EffectPayload) -> Self {
        Self {
            target,
            effect,
            payload,
        }
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_effect(&self) -> &Arc<GameplayEffect> {
        &self.effect
    }

    pub fn get_payload(&self) -> &EffectPayload {
        &self.payload
    }
}

#[derive(Resource, Default)]
pub struct GameplayEffectApplicationQueue {
    requests: VecDeque<GameplayEffectApplicationRequest>,
}

impl GameplayEffectApplicationQueue {
    pub fn push(&mut self, request: GameplayEffectApplicationRequest) {
        self.requests.push_back(request);
    }

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

    pub fn pop(&mut self) -> Option<GameplayEffectApplicationRequest> {
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
}

pub fn process_gameplay_effect_application_queue_system(
    mut effect_queue: ResMut<GameplayEffectApplicationQueue>,
    mut params: AbilitySystemParams,
) {
    while let Some(request) = effect_queue.pop() {
        if let Err(error) = apply_gameplay_effect(
            request.get_target(),
            request.get_effect(),
            &mut params,
            request.get_payload(),
        ) {
            if error.is_rejection() {
                debug!("queued gameplay effect was rejected: {error}");
            } else {
                error!("queued gameplay effect application failed: {error}");
            }
        }
    }
}

pub fn gameplay_effect_application_queue_has_work(
    queue: Option<Res<GameplayEffectApplicationQueue>>,
) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}
