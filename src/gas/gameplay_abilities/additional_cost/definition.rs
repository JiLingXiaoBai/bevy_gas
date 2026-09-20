use crate::unique_names::UniqueName;
use std::borrow::Cow;
use std::error::Error;
use std::fmt;
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

/// A configuration error, gameplay rejection, or failure of an external-resource provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalCostError {
    /// A requirement requested zero units.
    InvalidAmount,
    /// Additional requirements were supplied without configuring a provider.
    MissingProvider,
    /// The configured provider does not recognize this resource.
    UnsupportedResource {
        /// Identifier the provider cannot price or debit.
        resource: UniqueName,
    },
    /// The complete batch requires more units than are available.
    InsufficientResource {
        /// Resource whose available balance is insufficient.
        resource: UniqueName,
        /// Total required by the batch, including repeated entries.
        required: u32,
        /// Units currently available for payment.
        available: u32,
    },
    /// A game rule prevented payment without changing external state.
    Rejected {
        /// Game-specific explanation of the refusal.
        reason: Cow<'static, str>,
    },
    /// Provider state, arithmetic, or compensation failed.
    ProviderFailure {
        /// Diagnostic explanation of the provider failure.
        reason: Cow<'static, str>,
    },
}

impl AdditionalCostError {
    /// Returns whether the error is an expected gameplay rejection.
    pub fn is_rejection(&self) -> bool {
        matches!(
            self,
            Self::InsufficientResource { .. } | Self::Rejected { .. }
        )
    }
}

impl fmt::Display for AdditionalCostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAmount => write!(f, "additional cost amount must be positive"),
            Self::MissingProvider => write!(f, "no additional-cost provider is configured"),
            Self::UnsupportedResource { resource } => {
                write!(f, "unsupported additional-cost resource {resource:?}")
            }
            Self::InsufficientResource {
                resource,
                required,
                available,
            } => write!(
                f,
                "additional-cost resource {resource:?} requires {required} units but only {available} are available"
            ),
            Self::Rejected { reason } => write!(f, "additional cost rejected: {reason}"),
            Self::ProviderFailure { reason } => {
                write!(f, "additional-cost provider failed: {reason}")
            }
        }
    }
}

impl Error for AdditionalCostError {}
