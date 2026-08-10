use super::{
    AbilityActivationContext, AbilityActivationData, AbilityChainContext, AbilitySpecHandle,
};
use crate::gameplay_targeting::AbilityActivationTargets;
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
    spec_handle: AbilitySpecHandle,
    activation_data: AbilityActivationData,
    status: AbilityActivationStatus,
}

impl ActiveGameplayAbility {
    /// Creates active runtime state from separate activation inputs.
    ///
    /// This compatibility convenience constructor assembles an
    /// [`AbilityActivationData`] value before delegating to [`Self::from_data`].
    ///
    /// # Parameters
    ///
    /// - `source`: Entity that owns the activated ability.
    /// - `spec_handle`: Handle of the granted ability being executed.
    /// - `targets`: Target selection captured for the activation.
    /// - `status`: Initial runtime status of the activation.
    /// - `activation_context`: Metadata describing how the activation was initiated.
    ///
    /// # Returns
    ///
    /// Active runtime state containing the assembled activation data.
    pub fn new(
        source: Entity,
        spec_handle: AbilitySpecHandle,
        targets: impl Into<AbilityActivationTargets>,
        status: AbilityActivationStatus,
        activation_context: AbilityActivationContext,
    ) -> Self {
        Self::from_data(
            spec_handle,
            AbilityActivationData::new(source, targets, activation_context),
            status,
        )
    }

    /// Creates active runtime state from previously captured activation data.
    ///
    /// # Parameters
    ///
    /// - `spec_handle`: Handle of the granted ability being executed.
    /// - `activation_data`: Shared source, targets, and context for the activation.
    /// - `status`: Initial runtime status of the activation.
    ///
    /// # Returns
    ///
    /// Active runtime state that owns the supplied activation data.
    pub fn from_data(
        spec_handle: AbilitySpecHandle,
        activation_data: AbilityActivationData,
        status: AbilityActivationStatus,
    ) -> Self {
        Self {
            spec_handle,
            activation_data,
            status,
        }
    }

    /// Returns the shared data captured for this activation.
    pub fn get_activation_data(&self) -> &AbilityActivationData {
        &self.activation_data
    }

    pub fn get_source(&self) -> Entity {
        self.activation_data.get_source()
    }

    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.spec_handle
    }

    /// Returns the complete target selection captured for this activation.
    pub fn get_targets(&self) -> &AbilityActivationTargets {
        self.activation_data.get_targets()
    }

    /// Returns the primary target captured for this activation.
    pub fn get_target(&self) -> Entity {
        self.activation_data.get_target()
    }

    pub fn get_status(&self) -> AbilityActivationStatus {
        self.status
    }

    pub fn get_chain(&self) -> Option<&AbilityChainContext> {
        self.activation_data.get_context().get_chain()
    }

    pub fn get_activation_context(&self) -> &AbilityActivationContext {
        self.activation_data.get_context()
    }

    pub fn set_status(&mut self, status: AbilityActivationStatus) {
        self.status = status;
    }
}
