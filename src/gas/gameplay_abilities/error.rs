use super::AbilitySpecHandle;
use std::error::Error;
use std::fmt;

/// Describes why an ability activation chain could not be extended or validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityChainError {
    /// Extending or validating the path exceeded its configured depth limit.
    DepthExceeded { chain_id: u64, max_depth: u8 },
    /// The path contains a repeated ability handle.
    CycleDetected {
        chain_id: u64,
        handle: AbilitySpecHandle,
    },
    /// The path ends at a different ability handle than requested.
    HandleMismatch {
        chain_id: u64,
        expected: AbilitySpecHandle,
        actual: AbilitySpecHandle,
    },
    /// The path contains no ability handles.
    EmptyChain { chain_id: u64 },
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
