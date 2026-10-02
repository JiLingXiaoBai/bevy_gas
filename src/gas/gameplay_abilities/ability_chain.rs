use super::{AbilityChainError, AbilitySpecHandle};
use crate::settings::GameplayAbilitySystemSettings;

/// Tracks one activation path, counting child-activation edges from a depth-zero root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityChainContext {
    chain_id: u64,
    depth: u8,
    visited: Vec<AbilitySpecHandle>,
}

impl AbilityChainContext {
    /// Maximum number of child-activation edges from the root.
    pub const MAX_DEPTH: u8 = GameplayAbilitySystemSettings::ABILITY_CHAIN_MAX_DEPTH;

    /// Creates the root path for `handle` and `chain_id`, with depth zero.
    pub fn root(handle: AbilitySpecHandle, chain_id: u64) -> Self {
        Self {
            chain_id,
            depth: 0,
            visited: vec![handle],
        }
    }

    /// Returns a child path for `handle`, or an error if it repeats a handle or exceeds the limit.
    /// A path at [`Self::MAX_DEPTH`] is valid but cannot be extended.
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

    /// Validates that this path ends at `handle` without repetition or excessive depth.
    /// Returns an error for an empty path, a mismatched handle, a cycle, or depth above the limit.
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

    /// Returns the identifier assigned to the root activation.
    pub fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Returns the number of child-activation edges from the root.
    pub fn get_depth(&self) -> u8 {
        self.depth
    }

    /// Returns the visited handles in activation order, including the root.
    pub fn get_visited(&self) -> &[AbilitySpecHandle] {
        &self.visited
    }
}
