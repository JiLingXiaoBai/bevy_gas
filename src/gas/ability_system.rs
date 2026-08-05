//! Ability-system component storage, activation, commit, and lifecycle orchestration.

mod activation;
mod commit;
mod component;
mod lifecycle;
mod params;

pub use crate::gameplay_execution::AbilityActivationRequest;
pub(crate) use activation::execute_ability_activation_in_batch;
pub use activation::{
    AbilityActivationError, can_activate_ability, try_activate_ability_by_handle,
};
pub use commit::{AbilityCommitError, commit_ability};
pub use component::{AbilitySystemComponent, GameplayAbilitySystemBundle};
pub use lifecycle::{cancel_ability, cleanup_finished_abilities_system, end_ability};
pub use params::{AbilitySystemParams, PendingActiveGameplayAbilities};
