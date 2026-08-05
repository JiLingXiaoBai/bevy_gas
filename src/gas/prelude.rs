//! Commonly used Gameplay Ability System types.
//!
//! The prelude is intentionally small. Specialized systems, errors, and
//! low-level runtime types remain available from their owning domain modules.

pub use super::ability_system::{
    AbilitySystemComponent, AbilitySystemParams, GameplayAbilitySystemBundle,
};
pub use super::attributes::{AttributeId, AttributeIdRegister, AttributeRegion, AttributeSet};
pub use super::gameplay_abilities::{
    AbilityActivationContext, AbilitySpecHandle, AbilityTags, GameplayAbility,
};
pub use super::gameplay_effects::{
    ActiveGameplayEffects, EffectDurationTicks, EffectPayload, EffectSystemParams, EffectTags,
    GameplayEffect, StackingPolicy,
};
pub use super::gameplay_execution::GameplayExecutionQueue;
pub use super::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagRegister, TagRequirements,
};
pub use super::gameplay_targeting::{
    AbilityTargetData, TargetingDefinition, TargetingRequestQueue,
};
pub use super::modifiers::{
    Modifier, ModifierEvaluationContext, ModifierMagnitude, ModifierMagnitudeCalculation,
    ModifierOperation,
};
pub use super::runtime_plugin::{
    GameplayAbilitySystemPlugin, GameplayAbilitySystemRuntimePlugin, GameplayAbilitySystemSet,
};
