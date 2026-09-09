use super::AbilitySpecHandle;
use crate::settings::GameplayAbilitySystemSettings;
use std::error::Error;
use std::fmt;

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
