//! Gameplay ability definitions, active instances, and tick-driven tasks.
//!
//! Ability definitions describe activation policy and startup tasks; runtime
//! activation and commit orchestration is owned by the ability-system module.

mod ability_task;
mod active_gameplay_ability;
mod gameplay_ability;
mod gameplay_ability_spec;

pub use ability_task::{
    AbilityTask, AbilityTaskDef, AbilityTaskEvent, AbilityTaskKind, AbilityTaskOnFinished,
    AbilityTaskOnFinishedDef, tick_ability_tasks_system,
};
pub(crate) use ability_task::{AbilityTaskCompletion, dispatch_ability_task_completion};
pub use active_gameplay_ability::{
    AbilityActivationContext, AbilityActivationReason, AbilityActivationStatus,
    AbilityChainContext, AbilityChainError, ActiveAbilityHandle, ActiveGameplayAbility,
};
pub use gameplay_ability::{AbilityTags, GameplayAbility};
pub use gameplay_ability_spec::{AbilitySpecHandle, GameplayAbilitySpec};
