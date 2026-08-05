use super::{EffectPayload, GameplayEffect};
use bevy::prelude::*;
use std::sync::Arc;

/// Captured input for one queued gameplay-effect application.
#[derive(Clone)]
pub struct GameplayEffectApplicationRequest {
    target: Entity,
    effect: Arc<GameplayEffect>,
    payload: EffectPayload,
}

impl GameplayEffectApplicationRequest {
    /// Creates an effect application request.
    pub fn new(target: Entity, effect: Arc<GameplayEffect>, payload: EffectPayload) -> Self {
        Self {
            target,
            effect,
            payload,
        }
    }

    /// Returns the effect target.
    pub fn get_target(&self) -> Entity {
        self.target
    }

    /// Returns the shared effect definition.
    pub fn get_effect(&self) -> &Arc<GameplayEffect> {
        &self.effect
    }

    /// Returns the captured application payload.
    pub fn get_payload(&self) -> &EffectPayload {
        &self.payload
    }
}
