use super::{AbilityChainContext, AbilityChainError, AbilitySpecHandle, ActiveAbilityHandle};
use crate::attributes::AttributeSetSnapshot;
use crate::gameplay_effects::EffectPayload;
use crate::unique_names::UniqueName;
use bevy::prelude::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationReason {
    Direct,
    /// Activation requested by an application-defined input action.
    Input,
    Chained {
        parent_ability: ActiveAbilityHandle,
    },
    TaskEvent {
        event_id: UniqueName,
    },
    GameplayEffect,
}

#[derive(Clone)]
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    reason: AbilityActivationReason,
}

impl AbilityActivationContext {
    /// Creates a direct activation context using `source` as the default instigator.
    pub fn direct(source: Entity, chain: AbilityChainContext) -> Self {
        Self {
            chain: Some(chain),
            instigator: source,
            causer: None,
            source_snapshot: None,
            reason: AbilityActivationReason::Direct,
        }
    }

    /// Creates an input activation context using `source` as the default instigator.
    ///
    /// `chain` identifies the queued ability execution chain. Returns a context with reason
    /// [`AbilityActivationReason::Input`]; device and logical-action metadata stay in the input layer.
    pub fn input(source: Entity, chain: AbilityChainContext) -> Self {
        Self {
            reason: AbilityActivationReason::Input,
            ..Self::direct(source, chain)
        }
    }

    /// Sets the optional physical entity that directly caused this ability activation.
    pub fn with_causer(mut self, causer: Option<Entity>) -> Self {
        self.causer = causer;
        self
    }

    /// Sets the entity that initiated this ability activation.
    pub fn with_instigator(mut self, instigator: Entity) -> Self {
        self.instigator = instigator;
        self
    }

    pub fn with_source_snapshot(mut self, source_snapshot: AttributeSetSnapshot) -> Self {
        self.source_snapshot = Some(source_snapshot);
        self
    }

    pub fn child_for_chained_ability(
        &self,
        parent_ability: ActiveAbilityHandle,
        handle: AbilitySpecHandle,
    ) -> Result<Self, AbilityChainError> {
        let Some(chain) = &self.chain else {
            return Err(AbilityChainError::EmptyChain { chain_id: 0 });
        };

        Ok(Self {
            chain: Some(chain.next(handle)?),
            instigator: self.instigator,
            causer: self.causer,
            source_snapshot: self.source_snapshot.clone(),
            reason: AbilityActivationReason::Chained { parent_ability },
        })
    }

    pub fn get_chain(&self) -> Option<&AbilityChainContext> {
        self.chain.as_ref()
    }

    pub fn get_instigator(&self) -> Entity {
        self.instigator
    }

    /// Returns the optional physical entity that directly caused this ability activation.
    pub fn get_causer(&self) -> Option<Entity> {
        self.causer
    }

    pub fn get_source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.source_snapshot.as_ref()
    }

    pub fn get_reason(&self) -> AbilityActivationReason {
        self.reason
    }
}

/// Builds the canonical effect payload for an ability execution path.
pub(crate) fn effect_payload_from_ability_context(
    source: Entity,
    level: u32,
    activation_context: Option<&AbilityActivationContext>,
) -> EffectPayload {
    let Some(activation_context) = activation_context else {
        return EffectPayload::new(source, None, level);
    };

    let payload = EffectPayload::new(source, activation_context.get_causer(), level)
        .with_instigator(activation_context.get_instigator());
    if let Some(source_snapshot) = activation_context.get_source_snapshot() {
        payload.with_source_snapshot(source_snapshot.clone())
    } else {
        payload
    }
}
