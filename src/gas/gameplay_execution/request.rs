use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityActivationData, AbilitySpecHandle,
};
use crate::gameplay_effects::{EffectPayload, GameplayEffect};
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::*;
use std::sync::Arc;

/// Canonical captured input for one ability activation.
///
/// Queued and synchronous activation paths pass this request unchanged through validation and
/// ability startup so the two paths share the same source, targets, handle, and context semantics.
#[derive(Clone)]
pub struct AbilityActivationRequest {
    handle: AbilitySpecHandle,
    activation_data: AbilityActivationData,
}

impl AbilityActivationRequest {
    /// Creates an activation request.
    pub fn new(
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        handle: AbilitySpecHandle,
        context: AbilityActivationContext,
    ) -> Self {
        Self::from_data(handle, AbilityActivationData::new(source, targets, context))
    }

    /// Creates an activation request from previously captured activation data.
    ///
    /// # Parameters
    ///
    /// - `handle`: Handle of the granted ability to activate.
    /// - `activation_data`: Shared source, targets, and context for the activation.
    ///
    /// # Returns
    ///
    /// A request that owns the supplied activation data.
    pub fn from_data(handle: AbilitySpecHandle, activation_data: AbilityActivationData) -> Self {
        Self {
            handle,
            activation_data,
        }
    }

    /// Returns the shared data captured for this activation.
    pub fn get_activation_data(&self) -> &AbilityActivationData {
        &self.activation_data
    }

    /// Returns the ability owner.
    pub fn get_source(&self) -> Entity {
        self.activation_data.get_source()
    }

    /// Returns the complete captured target selection.
    pub fn get_targets(&self) -> &AbilityActivationTargets {
        self.activation_data.get_targets()
    }

    /// Returns the captured primary target.
    pub fn get_target(&self) -> Entity {
        self.activation_data.get_target()
    }

    /// Returns the granted ability handle.
    pub fn get_handle(&self) -> AbilitySpecHandle {
        self.handle
    }

    /// Returns the captured activation context.
    pub fn get_context(&self) -> &AbilityActivationContext {
        self.activation_data.get_context()
    }
}

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

/// One deterministic gameplay mutation request.
#[derive(Clone)]
pub enum GameplayExecutionRequest {
    /// Starts an ability after validating its captured activation context.
    ActivateAbility(AbilityActivationRequest),
    /// Applies a gameplay effect with its captured payload.
    ApplyGameplayEffect(GameplayEffectApplicationRequest),
}

impl From<AbilityActivationRequest> for GameplayExecutionRequest {
    fn from(value: AbilityActivationRequest) -> Self {
        Self::ActivateAbility(value)
    }
}

impl From<GameplayEffectApplicationRequest> for GameplayExecutionRequest {
    fn from(value: GameplayEffectApplicationRequest) -> Self {
        Self::ApplyGameplayEffect(value)
    }
}
