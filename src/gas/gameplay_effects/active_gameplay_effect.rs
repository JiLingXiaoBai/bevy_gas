//! Active gameplay-effect state, application, execution, cleanup, and ticking.

mod application;
mod execution;
mod modifiers;
mod planning;
mod removal;
mod requirements;
mod state;
mod ticking;

pub use application::apply_gameplay_effect;
pub(crate) use application::apply_gameplay_effect_in_batch;
pub use execution::execute_gameplay_effect_plan;
pub(crate) use execution::{execute_gameplay_effect_plan_in_batch, validate_gameplay_effect_plan};
pub use planning::{
    GameplayEffectApplicationError, GameplayEffectApplicationPlan, prepare_gameplay_effect,
};
pub use removal::{
    get_active_effects_on_target, has_active_effect_with_tags, remove_active_effect,
    remove_active_effects_with_tags,
};
pub(crate) use requirements::{
    ActiveEffectRequirementSync, resolve_active_effect_tag_requirements_if_dirty,
};
pub use requirements::{
    resolve_active_effect_tag_requirements, update_active_effect_tag_requirements_system,
};
pub use state::{
    ActiveEffectDurationTicks, ActiveEffectHandle, ActiveEffectPeriodTicks, ActiveGameplayEffect,
    ActiveGameplayEffects,
};
pub use ticking::{tick_effect_duration_system, tick_effect_period_system};
