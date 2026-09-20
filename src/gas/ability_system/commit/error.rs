//! Structured commit failures and gameplay-rejection classification.

use crate::gameplay_abilities::AdditionalCostError;
use crate::gameplay_effects::GameplayEffectApplicationError;
use std::error::Error;
use std::fmt;

/// Describes why an ability cost or cooldown could not be committed.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityCommitError {
    /// External-resource validation or temporary payment failed.
    AdditionalCost(AdditionalCostError),
    /// External-resource compensation failed after the original GAS commit error.
    AdditionalCostRollback {
        /// The error that caused external payment to be compensated.
        commit_error: Box<AbilityCommitError>,
        /// The provider's error while restoring its external resources.
        rollback_error: AdditionalCostError,
    },
    /// A cost definition contains an operation other than addition.
    CostModifiersMustBeAdditive,
    /// The cost effect could not be prepared.
    CostPreparation(GameplayEffectApplicationError),
    /// A cost definition is not instant.
    CostMustBeInstant,
    /// A cost magnitude or projected base is non-finite, or projected current is non-finite or negative.
    InsufficientCost,
    /// The cooldown effect could not be prepared.
    CooldownPreparation(GameplayEffectApplicationError),
    /// The prepared cost plan could not be executed.
    CostExecution(GameplayEffectApplicationError),
    /// The prepared cooldown plan could not be executed.
    CooldownExecution(GameplayEffectApplicationError),
}

impl fmt::Display for AbilityCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdditionalCost(error) => write!(f, "additional cost failed: {error}"),
            Self::AdditionalCostRollback {
                commit_error,
                rollback_error,
            } => write!(
                f,
                "{commit_error}; additional-cost rollback also failed: {rollback_error}"
            ),
            Self::CostModifiersMustBeAdditive => {
                write!(f, "ability cost may contain only additive modifiers")
            }
            Self::CostPreparation(error) => write!(f, "ability cost preparation failed: {error}"),
            Self::CostMustBeInstant => write!(f, "ability cost must be an instant effect"),
            Self::InsufficientCost => write!(f, "ability source cannot pay the prepared cost"),
            Self::CooldownPreparation(error) => {
                write!(f, "ability cooldown preparation failed: {error}")
            }
            Self::CostExecution(error) => write!(f, "ability cost execution failed: {error}"),
            Self::CooldownExecution(error) => {
                write!(f, "ability cooldown execution failed: {error}")
            }
        }
    }
}

impl Error for AbilityCommitError {}

impl AbilityCommitError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        match self {
            Self::AdditionalCost(error) => error.is_rejection(),
            Self::AdditionalCostRollback { .. } => false,
            Self::CostPreparation(error) | Self::CooldownPreparation(error) => error.is_rejection(),
            Self::InsufficientCost => true,
            Self::CostModifiersMustBeAdditive
            | Self::CostMustBeInstant
            | Self::CostExecution(_)
            | Self::CooldownExecution(_) => false,
        }
    }
}
