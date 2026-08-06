use crate::support_test::{
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

#[path = "effects_test/application_test.rs"]
mod application_test;
#[path = "effects_test/removal_test.rs"]
mod removal_test;
#[path = "effects_test/requirements_test.rs"]
mod requirements_test;
#[path = "effects_test/stacking_test.rs"]
mod stacking_test;
#[path = "effects_test/ticking_test.rs"]
mod ticking_test;
