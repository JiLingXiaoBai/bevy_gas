//! Gameplay Ability System domains and fixed-tick runtime orchestration.

pub mod ability_system;
pub mod attributes;
pub mod gameplay_abilities;
pub mod gameplay_effects;
pub mod gameplay_execution;
pub mod gameplay_tags;
pub mod gameplay_targeting;
pub mod modifiers;
pub mod prelude;
mod runtime_plugin;
pub mod settings;

pub use ability_system::{
    AbilityActivationError, AbilityCommitError, AbilitySystemComponent, AbilitySystemParams,
    GameplayAbilitySystemBundle, PendingActiveGameplayAbilities, can_activate_ability,
    cancel_ability, cleanup_finished_abilities_system, commit_ability, end_ability,
    try_activate_ability_by_handle,
};
pub use attributes::{
    ATTRIBUTE_SET_SIZE, Aggregator, AttributeId, AttributeIdError, AttributeIdManager,
    AttributeIdRegister, AttributeLocation, AttributePostExecute, AttributeRegion, AttributeSet,
    AttributeSetError, AttributeSetSnapshot, AttributeSnapshot, COLD_ATTRIBUTE_SET_SIZE,
    HOT_ATTRIBUTE_SET_SIZE, default_executor, recalculate_attribute_sets_system,
};
pub use gameplay_abilities::{
    AbilityActivationContext, AbilityActivationReason, AbilityActivationStatus,
    AbilityChainContext, AbilityChainError, AbilitySpecHandle, AbilityTags, AbilityTask,
    AbilityTaskDef, AbilityTaskEvent, AbilityTaskKind, AbilityTaskOnFinished,
    AbilityTaskOnFinishedDef, ActiveAbilityHandle, ActiveGameplayAbility, GameplayAbility,
    GameplayAbilitySpec, tick_ability_tasks_system,
};
pub use gameplay_effects::{
    ActiveEffectDurationTicks, ActiveEffectHandle, ActiveEffectPeriodTicks, ActiveGameplayEffect,
    ActiveGameplayEffects, EffectContext, EffectDurationTicks, EffectDurationTicksSpec,
    EffectPayload, EffectPeriodTicks, EffectPeriodTicksSpec, EffectSystemParams, EffectTags,
    GameplayEffect, GameplayEffectApplicationError, GameplayEffectApplicationPlan,
    GameplayEffectImmunityQuery, GameplayEffectSpec, StackDurationPolicy, StackExpirationPolicy,
    StackMagnitudePolicy, StackOverflowPolicy, StackPeriodPolicy, StackingPolicy, StackingType,
    apply_gameplay_effect, execute_gameplay_effect_plan, get_active_effects_on_target,
    has_active_effect_with_tags, prepare_gameplay_effect, remove_active_effect,
    remove_active_effects_with_tags, resolve_active_effect_tag_requirements,
    tick_effect_duration_system, tick_effect_period_system,
    update_active_effect_tag_requirements_system,
};
pub use gameplay_execution::{
    AbilityActivationRequest, GameplayEffectApplicationRequest, GameplayExecutionQueue,
    GameplayExecutionRequest, gameplay_execution_queue_has_work,
    process_gameplay_execution_queue_system,
};
pub use gameplay_tags::{
    BLOCK_SIZE_EXPONENT, GameplayTag, GameplayTagBits, GameplayTagContainer, GameplayTagError,
    GameplayTagManager, GameplayTagRegister, MAX_TAG_BLOCKS, MAX_TAG_COUNTS, TAG_BITS_PER_BLOCK,
    TagRequirements, add_bit_with_tag, tag_bits_from_tags, tag_bits_from_tags_with_manager,
};
pub use gameplay_targeting::{
    AbilityTargetData, AbilityTargetHit, Targetable, TargetingCandidateQuery,
    TargetingContinuation, TargetingDefinition, TargetingDefinitionError, TargetingError,
    TargetingInput, TargetingOperation, TargetingRequestId, TargetingRequestQueue,
    TargetingResultEvent, TargetingSortOrder, acquire_targets,
    process_targeting_request_queue_system, targeting_request_queue_has_work,
};
pub use modifiers::{
    AppliedModifier, Modifier, ModifierEvaluationContext, ModifierMagnitude,
    ModifierMagnitudeCalculation, ModifierOperation, ModifierSourceId, ModifierSpec,
};
pub use runtime_plugin::{
    GameplayAbilitySystemPlugin, GameplayAbilitySystemRuntimePlugin, GameplayAbilitySystemSet,
    GameplayTagPlugin, RandomPlugin, UniqueNamePlugin,
};
pub use settings::GameplayAbilitySystemSettings;
