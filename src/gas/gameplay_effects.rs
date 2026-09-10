//! Gameplay-effect definitions, prepared specifications, and active runtime state.
//!
//! Immutable effect definitions are separated from target-owned active-effect
//! application, requirement convergence, removal, and ticking.

mod active_gameplay_effect;
mod effect_system_params;
mod gameplay_effect;
mod gameplay_effect_spec;

pub use crate::gameplay_execution::GameplayEffectApplicationRequest;
pub use active_gameplay_effect::{
    ActiveEffectDurationTicks, ActiveEffectHandle, ActiveEffectPeriodTicks, ActiveGameplayEffect,
    ActiveGameplayEffects, GameplayEffectApplicationError, GameplayEffectApplicationPlan,
    apply_gameplay_effect, execute_gameplay_effect_plan, get_active_effects_on_target,
    has_active_effect_with_tags, prepare_gameplay_effect, remove_active_effect,
    remove_active_effects_with_tags, resolve_active_effect_tag_requirements,
    tick_effect_duration_system, tick_effect_period_system,
    update_active_effect_tag_requirements_system,
};
pub(crate) use active_gameplay_effect::{
    ActiveEffectRequirementSync, apply_gameplay_effect_in_batch,
    execute_gameplay_effect_plan_in_batch, preview_instant_effect_modifiers,
    resolve_active_effect_tag_requirements_if_dirty, validate_gameplay_effect_plan,
};
pub use effect_system_params::EffectSystemParams;
pub use gameplay_effect::{
    EffectContext, EffectDurationTicks, EffectPayload, EffectPeriodTicks, EffectTags,
    GameplayEffect, GameplayEffectImmunityQuery, StackDurationPolicy, StackExpirationPolicy,
    StackMagnitudePolicy, StackOverflowPolicy, StackPeriodPolicy, StackingPolicy, StackingType,
    TagRequirements,
};
pub use gameplay_effect_spec::{
    EffectDurationTicksSpec, EffectPeriodTicksSpec, GameplayEffectSpec,
};
