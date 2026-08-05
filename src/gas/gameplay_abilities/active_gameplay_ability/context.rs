use super::{AbilityChainContext, AbilityChainError, AbilitySpecHandle, ActiveAbilityHandle};
use crate::attributes::AttributeSetSnapshot;
use crate::gameplay_targeting::AbilityTargetData;
use crate::unique_names::UniqueName;
use bevy::prelude::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationReason {
    Direct,
    Input { input_id: u16 },
    Chained { parent_ability: ActiveAbilityHandle },
    TaskEvent { event_id: UniqueName },
    GameplayEffect,
}

#[derive(Clone)]
pub struct AbilityActivationContext {
    chain: Option<AbilityChainContext>,
    instigator: Entity,
    causer: Option<Entity>,
    source_snapshot: Option<AttributeSetSnapshot>,
    target_data: Option<AbilityTargetData>,
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
            target_data: None,
            reason: AbilityActivationReason::Direct,
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

    /// Attaches the target data acquired for this activation.
    pub fn with_target_data(mut self, target_data: AbilityTargetData) -> Self {
        self.target_data = Some(target_data);
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
            target_data: self.target_data.clone(),
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

    /// Returns target data acquired before or during this activation.
    pub fn get_target_data(&self) -> Option<&AbilityTargetData> {
        self.target_data.as_ref()
    }

    pub fn get_reason(&self) -> AbilityActivationReason {
        self.reason
    }
}
