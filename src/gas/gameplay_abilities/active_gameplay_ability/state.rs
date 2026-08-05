use super::{AbilityActivationContext, AbilityChainContext, AbilitySpecHandle};
use bevy::prelude::{Component, Entity};

pub type ActiveAbilityHandle = Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationStatus {
    Active,
    Ending,
    Cancelled,
}

#[derive(Component, Clone)]
pub struct ActiveGameplayAbility {
    source: Entity,
    spec_handle: AbilitySpecHandle,
    target: Entity,
    status: AbilityActivationStatus,
    activation_context: AbilityActivationContext,
}

impl ActiveGameplayAbility {
    pub fn new(
        source: Entity,
        spec_handle: AbilitySpecHandle,
        target: Entity,
        status: AbilityActivationStatus,
        activation_context: AbilityActivationContext,
    ) -> Self {
        Self {
            source,
            spec_handle,
            target,
            status,
            activation_context,
        }
    }

    pub fn get_source(&self) -> Entity {
        self.source
    }

    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.spec_handle
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_status(&self) -> AbilityActivationStatus {
        self.status
    }

    pub fn get_chain(&self) -> Option<&AbilityChainContext> {
        self.activation_context.get_chain()
    }

    pub fn get_activation_context(&self) -> &AbilityActivationContext {
        &self.activation_context
    }

    pub fn set_status(&mut self, status: AbilityActivationStatus) {
        self.status = status;
    }
}
