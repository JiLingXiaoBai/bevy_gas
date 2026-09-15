//! Ability instance bookkeeping, state transitions, and ECS lifetime cleanup.

use super::component::AbilitySystemComponent;
use super::params::{AbilitySystemParams, PendingActiveGameplayAbilities};
use crate::gameplay_abilities::{
    AbilityActivationStatus, ActiveAbilityHandle, ActiveGameplayAbility, GameplayAbility,
};
use crate::gameplay_execution::AbilityActivationRequest;
use crate::gameplay_tags::{
    GameplayTag, GameplayTagError, GameplayTagManager, tag_bits_from_tags_with_manager,
};

mod cleanup;
mod instances;
mod transitions;

pub use cleanup::cleanup_finished_abilities_system;
pub(crate) use cleanup::{cleanup_discarded_ability_system, cleanup_discarded_active_ability};
pub use transitions::{cancel_ability, end_ability};
pub(super) use transitions::{cancel_active_abilities_with_tags, finish_ability_with_status};

pub(super) use instances::finish_ability_startup;
