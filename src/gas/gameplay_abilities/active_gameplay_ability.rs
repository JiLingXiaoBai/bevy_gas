use super::AbilitySpecHandle;
use crate::attributes::AttributeSetSnapshot;
use crate::settings::GameplayAbilitySystemSettings;
use crate::unique_names::UniqueName;
use bevy::prelude::{Component, Entity};
use std::error::Error;
use std::fmt;

pub type ActiveAbilityHandle = Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityActivationStatus {
    Active,
    Ending,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityChainError {
    DepthExceeded {
        chain_id: u64,
        max_depth: u8,
    },
    CycleDetected {
        chain_id: u64,
        handle: AbilitySpecHandle,
    },
    HandleMismatch {
        chain_id: u64,
        expected: AbilitySpecHandle,
        actual: AbilitySpecHandle,
    },
    EmptyChain {
        chain_id: u64,
    },
}

impl fmt::Display for AbilityChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbilityChainError::DepthExceeded {
                chain_id,
                max_depth,
            } => write!(f, "ability chain {chain_id} exceeded max depth {max_depth}"),
            AbilityChainError::CycleDetected { chain_id, handle } => write!(
                f,
                "ability chain {chain_id} detected cycle at handle {}",
                handle.get_value()
            ),
            AbilityChainError::HandleMismatch {
                chain_id,
                expected,
                actual,
            } => write!(
                f,
                "ability chain {chain_id} handle mismatch: expected {}, got {}",
                expected.get_value(),
                actual.get_value()
            ),
            AbilityChainError::EmptyChain { chain_id } => {
                write!(f, "ability chain {chain_id} has no visited handles")
            }
        }
    }
}

impl Error for AbilityChainError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityChainContext {
    chain_id: u64,
    depth: u8,
    visited: Vec<AbilitySpecHandle>,
}

impl AbilityChainContext {
    pub const MAX_DEPTH: u8 = GameplayAbilitySystemSettings::ABILITY_CHAIN_MAX_DEPTH;

    pub fn root(handle: AbilitySpecHandle, chain_id: u64) -> Self {
        Self {
            chain_id,
            depth: 0,
            visited: vec![handle],
        }
    }

    pub fn next(&self, handle: AbilitySpecHandle) -> Result<Self, AbilityChainError> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(AbilityChainError::DepthExceeded {
                chain_id: self.chain_id,
                max_depth: Self::MAX_DEPTH,
            });
        }

        if self.visited.contains(&handle) {
            return Err(AbilityChainError::CycleDetected {
                chain_id: self.chain_id,
                handle,
            });
        }

        let mut visited = self.visited.clone();
        visited.push(handle);

        Ok(Self {
            chain_id: self.chain_id,
            depth: self.depth.saturating_add(1),
            visited,
        })
    }

    pub fn validate_for_handle(&self, handle: AbilitySpecHandle) -> Result<(), AbilityChainError> {
        let Some(&current_handle) = self.visited.last() else {
            return Err(AbilityChainError::EmptyChain {
                chain_id: self.chain_id,
            });
        };

        if current_handle != handle {
            return Err(AbilityChainError::HandleMismatch {
                chain_id: self.chain_id,
                expected: current_handle,
                actual: handle,
            });
        }

        if self.visited[..self.visited.len().saturating_sub(1)].contains(&handle) {
            return Err(AbilityChainError::CycleDetected {
                chain_id: self.chain_id,
                handle,
            });
        }

        if self.depth > Self::MAX_DEPTH {
            return Err(AbilityChainError::DepthExceeded {
                chain_id: self.chain_id,
                max_depth: Self::MAX_DEPTH,
            });
        }

        Ok(())
    }

    pub fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn get_depth(&self) -> u8 {
        self.depth
    }

    pub fn get_visited(&self) -> &[AbilitySpecHandle] {
        &self.visited
    }
}

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
