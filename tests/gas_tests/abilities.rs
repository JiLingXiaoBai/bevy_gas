use crate::support::{
    ability_task_count, activate_ability, activate_ability_result, activate_ability_with_context,
    active_ability_context_for_spec, active_ability_count, active_ability_entity_for_spec,
    add_tag_to_entity, attribute_set, current_value, effect_tags, empty_effect_tags, give_ability,
    instant_add_effect, register_attribute, register_tag, run_ability_tasks,
    run_finished_ability_cleanup, run_gameplay_execution_queue, spawn_ability_task,
    spawn_active_ability, spawn_attribute_set, test_app,
};
use bevy::prelude::*;
use bevy_tools::{
    AbilityActivationContext, AbilityActivationError, AbilityActivationReason,
    AbilityActivationStatus, AbilityChainContext, AbilityChainError, AbilitySpecHandle,
    AbilitySystemComponent, AbilityTags, AbilityTask, AbilityTaskDef, AbilityTaskOnFinished,
    AbilityTaskOnFinishedDef, AttributeId, EffectDurationTicks, GameplayAbility,
    GameplayAbilitySpec, GameplayAbilitySystemBundle, GameplayEffect, GameplayExecutionQueue,
    GameplayTagContainer, Modifier, ModifierEvaluationContext, ModifierMagnitude,
    ModifierMagnitudeCalculation, ModifierOperation, StackingPolicy,
};
use std::sync::Arc;

struct ContextPayloadMagnitude {
    expected_instigator: Entity,
    expected_causer: Entity,
    snapshot_attribute: AttributeId,
}

impl ModifierMagnitudeCalculation for ContextPayloadMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        if context.instigator() != self.expected_instigator
            || context.causer() != Some(self.expected_causer)
        {
            return 0.0;
        }

        context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get_current_value(context.attribute_id_manager(), self.snapshot_attribute)
                    .ok()
                    .flatten()
            })
            .unwrap_or(0.0)
    }
}

#[path = "abilities/activation.rs"]
mod activation;
#[path = "abilities/chaining.rs"]
mod chaining;
#[path = "abilities/commit.rs"]
mod commit;
#[path = "abilities/lifecycle.rs"]
mod lifecycle;
#[path = "abilities/tasks.rs"]
mod tasks;
