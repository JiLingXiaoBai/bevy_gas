use crate::gameplay_abilities::{AbilityActivationContext, AbilitySpecHandle};
use bevy::prelude::*;

/// Captured input for one queued ability activation.
#[derive(Clone)]
pub struct AbilityActivationRequest {
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    context: AbilityActivationContext,
}

impl AbilityActivationRequest {
    /// Creates an activation request.
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

    /// Returns the ability owner.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the captured primary target.
    pub fn get_target(&self) -> Entity {
        self.target
    }

    /// Returns the granted ability handle.
    pub fn get_handle(&self) -> AbilitySpecHandle {
        self.handle
    }

    /// Returns the captured activation context.
    pub fn get_context(&self) -> &AbilityActivationContext {
        &self.context
    }
}
