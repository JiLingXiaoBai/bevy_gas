//! Gameplay ability definitions, active instances, and tick-driven tasks.
//!
//! Ability definitions describe activation policy and startup tasks; runtime
//! activation and commit orchestration is owned by the ability-system module.

mod ability_chain;
mod ability_task;
mod activation_context;
mod activation_data;
mod active_gameplay_ability;
mod gameplay_ability;
mod gameplay_ability_spec;

pub use ability_chain::{AbilityChainContext, AbilityChainError};
pub use ability_task::{
    AbilityTask, AbilityTaskDef, AbilityTaskEvent, AbilityTaskExecutionContext, AbilityTaskKind,
    AbilityTaskOnFinished, AbilityTaskOnFinishedDef, tick_ability_tasks_system,
};
pub(crate) use ability_task::{AbilityTaskCompletion, dispatch_ability_task_completion};
pub(crate) use activation_context::effect_payload_from_ability_context;
pub use activation_context::{AbilityActivationContext, AbilityActivationReason};
pub use activation_data::AbilityActivationData;
pub use active_gameplay_ability::{
    AbilityActivationStatus, ActiveAbilityHandle, ActiveGameplayAbility,
};
pub use gameplay_ability::{AbilityTags, GameplayAbility};
pub use gameplay_ability_spec::{AbilitySpecHandle, GameplayAbilitySpec};
