//! ECS-first Gameplay Ability System building blocks for Bevy.
//!
//! The crate exposes gameplay tags, attributes, effects, abilities, targeting,
//! and a deterministic fixed-tick execution pipeline through domain modules and
//! compatibility re-exports at the crate root.

#[cfg(feature = "luban-config")]
pub mod config;
pub mod gas;
mod randoms;
mod unique_names;

pub use gas::{
    ATTRIBUTE_SET_SIZE, AbilityActivationContext, AbilityActivationData, AbilityActivationError,
    AbilityActivationReason, AbilityActivationRequest, AbilityActivationStatus,
    AbilityActivationTargets, AbilityActivationTargetsError, AbilityChainContext,
    AbilityChainError, AbilityCommitError, AbilityInputBindingError, AbilityInputBindings,
    AbilitySpecHandle, AbilitySystemComponent, AbilitySystemParams, AbilityTags, AbilityTargetData,
    AbilityTargetHit, AbilityTask, AbilityTaskDef, AbilityTaskEvent, AbilityTaskExecutionContext,
    AbilityTaskKind, AbilityTaskOnFinished, AbilityTaskOnFinishedDef, ActiveAbilityHandle,
    ActiveEffectDurationTicks, ActiveEffectHandle, ActiveEffectPeriodTicks, ActiveGameplayAbility,
    ActiveGameplayEffect, ActiveGameplayEffects, Aggregator, AppliedModifier, AttributeId,
    AttributeIdError, AttributeIdManager, AttributeIdRegister, AttributeLocation,
    AttributePostExecute, AttributeRegion, AttributeSet, AttributeSetError, AttributeSetSnapshot,
    AttributeSnapshot, BLOCK_SIZE_EXPONENT, COLD_ATTRIBUTE_SET_SIZE, EffectContext,
    EffectDurationTicks, EffectDurationTicksSpec, EffectPayload, EffectPeriodTicks,
    EffectPeriodTicksSpec, EffectSystemParams, EffectTags, GameplayAbility, GameplayAbilitySpec,
    GameplayAbilitySystemBundle, GameplayAbilitySystemPlugin, GameplayAbilitySystemRuntimePlugin,
    GameplayAbilitySystemSet, GameplayAbilitySystemSettings, GameplayEffect,
    GameplayEffectApplicationError, GameplayEffectApplicationPlan,
    GameplayEffectApplicationRequest, GameplayEffectImmunityQuery, GameplayEffectSpec,
    GameplayExecutionQueue, GameplayExecutionRequest, GameplayTag, GameplayTagBits,
    GameplayTagContainer, GameplayTagError, GameplayTagManager, GameplayTagPlugin,
    GameplayTagRegister, HOT_ATTRIBUTE_SET_SIZE, MAX_TAG_BLOCKS, MAX_TAG_COUNTS, Modifier,
    ModifierEvaluationContext, ModifierMagnitude, ModifierMagnitudeCalculation, ModifierOperation,
    ModifierSourceId, ModifierSpec, PendingActiveGameplayAbilities, RandomPlugin,
    StackDurationPolicy, StackExpirationPolicy, StackMagnitudePolicy, StackOverflowPolicy,
    StackPeriodPolicy, StackingPolicy, StackingType, TAG_BITS_PER_BLOCK, TagRequirements,
    Targetable, TargetingCandidateQuery, TargetingContinuation, TargetingDefinition,
    TargetingDefinitionError, TargetingError, TargetingInput, TargetingOperation,
    TargetingRequestId, TargetingRequestQueue, TargetingResultEvent, TargetingSortOrder,
    UniqueNamePlugin, ability_input, ability_system, acquire_targets, add_bit_with_tag,
    apply_gameplay_effect, attributes, can_activate_ability, cancel_ability,
    cleanup_finished_abilities_system, commit_ability, default_executor, end_ability,
    execute_gameplay_effect_plan, gameplay_abilities, gameplay_effects, gameplay_execution,
    gameplay_execution_queue_has_work, gameplay_tags, gameplay_targeting,
    get_active_effects_on_target, has_active_effect_with_tags, modifiers, prelude,
    prepare_gameplay_effect, process_gameplay_execution_queue_system,
    process_targeting_request_queue_system, recalculate_attribute_sets_system,
    remove_active_effect, remove_active_effects_with_tags, resolve_active_effect_tag_requirements,
    settings, tag_bits_from_tags, tag_bits_from_tags_with_manager,
    targeting_request_queue_has_work, tick_ability_tasks_system, tick_effect_duration_system,
    tick_effect_period_system, try_activate_ability_by_handle,
    update_active_effect_tag_requirements_system,
};
pub use randoms::Random;
pub use unique_names::{UniqueName, UniqueNameError, UniqueNamePool};
extern crate core;
