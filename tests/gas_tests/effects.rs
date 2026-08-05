use crate::support::{
    active_effect_handles, add_modifier, add_tag_to_entity, apply_effect, apply_effect_result,
    attribute_set, current_value, effect_tags, empty_effect_tags, register_attribute, register_tag,
    remove_tag_from_entity, run_effect_duration_tick, run_effect_period_tick,
    run_effect_tag_requirements_update, run_fixed_update, spawn_attribute_set, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy_tools::{
    AbilitySystemParams, ActiveGameplayEffects, AttributeSet, EffectDurationTicks, EffectPayload,
    EffectPeriodTicks, EffectTags, GameplayEffect, GameplayEffectApplicationError,
    GameplayEffectImmunityQuery, GameplayExecutionQueue, GameplayTag, GameplayTagContainer,
    Modifier, ModifierMagnitude, ModifierOperation, StackDurationPolicy, StackExpirationPolicy,
    StackMagnitudePolicy, StackOverflowPolicy, StackPeriodPolicy, StackingPolicy, StackingType,
    TagRequirements, apply_gameplay_effect, execute_gameplay_effect_plan, prepare_gameplay_effect,
};
use std::sync::Arc;

fn tags_with_requirements(
    granted_tags: Vec<GameplayTag>,
    target_ongoing_tags: TagRequirements,
    target_removal_tags: TagRequirements,
) -> EffectTags {
    EffectTags::new(
        Vec::new(),
        granted_tags,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        target_ongoing_tags,
        TagRequirements::default(),
        target_removal_tags,
        Vec::new(),
        Vec::new(),
    )
}

#[path = "effects/application.rs"]
mod application;
#[path = "effects/removal.rs"]
mod removal;
#[path = "effects/requirements.rs"]
mod requirements;
#[path = "effects/stacking.rs"]
mod stacking;
#[path = "effects/ticking.rs"]
mod ticking;
