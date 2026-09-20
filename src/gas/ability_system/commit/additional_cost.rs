//! Synchronous ECS adapters for external ability costs.

use crate::gameplay_abilities::{AbilityActivationContext, AdditionalCost, AdditionalCostError};
use bevy::ecs::system::{ReadOnlySystemParam, SystemParam, SystemParamItem};
use bevy::prelude::Entity;

/// Inputs shared by an external-cost batch and its compensation.
///
/// Standalone `commit_ability` and read-only previews have no activation context. The source is
/// always the paying ability owner, independently of the ability's target selection.
#[derive(Clone, Copy)]
pub struct AdditionalCostContext<'a> {
    /// Entity whose external resources pay for the ability.
    pub source: Entity,
    /// Granted ability level; providers may use it in game-specific pricing rules.
    pub level: u32,
    /// Captured activation metadata, when executing an activation.
    pub activation: Option<&'a AbilityActivationContext>,
}

/// Provides synchronous, explicitly declared ECS access to a complete external-cost batch.
///
/// Implement this on a game-defined `SystemParam` with static lifetimes. Configure the runtime
/// with the same provider type used in `AbilitySystemParams<P>` and
/// `AbilityActivationCheckParams<P>`. The default `()` provider rejects nonempty costs.
///
/// `check` and `check_readonly` must inspect the entire batch without mutation. `prepare` must
/// recheck the batch, including repeated resource identifiers, and temporarily debit resources
/// synchronously. On error it must leave external state unchanged. On success it returns an owned
/// receipt; dropping that receipt confirms payment and must not refund resources. GAS calls
/// `rollback` only when subsequent attribute-cost or cooldown execution fails, before startup.
/// No receipt survives the synchronous commit call. Cancellation after success does not refund.
///
/// Do not defer payment through Commands or messages. Do not reenter GAS, mutate GAS-owned state,
/// or perform irreversible side effects. Restrict parameter access to the external components
/// and resources required by this provider. GAS does not roll back its own earlier effect
/// mutations, and reports compensation failures separately from the original commit failure.
pub trait AdditionalCostProvider: SystemParam + 'static {
    /// Read-only ECS access used by UI and AI previews, including immutable World access.
    type ReadOnly: ReadOnlySystemParam + 'static;
    /// Owned compensation data for one successfully prepared batch.
    type Receipt;

    /// Checks `costs` against runtime parameters for `context` without modifying state.
    /// Returns success or a concrete unavailability/configuration error.
    fn check(
        params: &SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError>;

    /// Checks the same batch using read-only parameters for an advisory preview.
    /// Returns success or a concrete unavailability/configuration error without changing state.
    fn check_readonly(
        params: &SystemParamItem<'_, '_, Self::ReadOnly>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError>;

    /// Revalidates and temporarily debits the entire batch for `context`.
    /// Returns a compensation receipt on success; errors must leave external state unchanged.
    fn prepare(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<Self::Receipt, AdditionalCostError>;

    /// Restores resources described by `receipt` after the associated GAS commit failed.
    /// Returns success after full compensation, or a concrete compensation failure.
    fn rollback(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        receipt: Self::Receipt,
    ) -> Result<(), AdditionalCostError>;
}

impl AdditionalCostProvider for () {
    type ReadOnly = ();
    type Receipt = ();

    fn check(
        _: &(),
        _: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        if costs.is_empty() {
            Ok(())
        } else {
            Err(AdditionalCostError::MissingProvider)
        }
    }

    fn check_readonly(
        params: &(),
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        Self::check(params, context, costs)
    }

    fn prepare(
        params: &mut (),
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        Self::check(params, context, costs)
    }

    fn rollback(
        _: &mut (),
        _: &AdditionalCostContext<'_>,
        _: (),
    ) -> Result<(), AdditionalCostError> {
        Ok(())
    }
}
