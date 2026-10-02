use super::AdditionalCostError;
use crate::unique_names::UniqueName;
use std::num::NonZeroU32;

/// One positive integer requirement interpreted by the game's additional-cost provider.
///
/// Resource identifiers belong to the game; GAS does not store inventory balances. Providers
/// must evaluate repeated identifiers together and reject unsupported identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalCost {
    resource: UniqueName,
    amount: NonZeroU32,
}

impl AdditionalCost {
    /// Creates a requirement for `amount` units of `resource`.
    ///
    /// Returns the requirement, or [`AdditionalCostError::InvalidAmount`] for zero units.
    pub fn new(resource: UniqueName, amount: u32) -> Result<Self, AdditionalCostError> {
        let amount = NonZeroU32::new(amount).ok_or(AdditionalCostError::InvalidAmount)?;
        Ok(Self { resource, amount })
    }

    /// Returns the game-defined resource identifier.
    pub fn resource(&self) -> UniqueName {
        self.resource
    }

    /// Returns the positive quantity requested by this requirement.
    pub fn amount(&self) -> u32 {
        self.amount.get()
    }
}
