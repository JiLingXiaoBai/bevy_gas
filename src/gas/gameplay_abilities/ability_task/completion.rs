use super::{
    AbilityActivationContext, AbilitySpecHandle, AbilityTaskOnFinished, ActiveAbilityHandle,
};
use crate::gameplay_effects::EffectPayload;
use crate::gameplay_execution::GameplayExecutionQueue;
use crate::unique_names::UniqueName;
use bevy::prelude::*;

#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    source: Entity,
    target: Entity,
    active_ability: ActiveAbilityHandle,
    spec_handle: AbilitySpecHandle,
    event_id: UniqueName,
    level: u32,
}

impl AbilityTaskEvent {
    pub fn new(
        source: Entity,
        target: Entity,
        active_ability: ActiveAbilityHandle,
        spec_handle: AbilitySpecHandle,
        event_id: UniqueName,
        level: u32,
    ) -> Self {
        Self {
            source,
            target,
            active_ability,
            spec_handle,
            event_id,
            level,
        }
    }

    pub fn get_source(&self) -> Entity {
        self.source
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.spec_handle
    }

    pub fn get_event_id(&self) -> UniqueName {
        self.event_id
    }

    pub fn get_level(&self) -> u32 {
        self.level
    }
}

pub(crate) enum AbilityTaskCompletion {
    Continue,
    EndAbility,
}

pub(crate) fn dispatch_ability_task_completion(
    active_ability: ActiveAbilityHandle,
    on_finished: AbilityTaskOnFinished,
    active_context: &AbilityActivationContext,
    commands: &mut Commands,
    execution_queue: &mut GameplayExecutionQueue,
) -> AbilityTaskCompletion {
    match on_finished {
        AbilityTaskOnFinished::None => {}
        AbilityTaskOnFinished::EndAbility => return AbilityTaskCompletion::EndAbility,
        AbilityTaskOnFinished::EmitEvent {
            source,
            target,
            spec_handle,
            event_id,
            level,
        } => {
            commands.trigger(AbilityTaskEvent::new(
                source,
                target,
                active_ability,
                spec_handle,
                event_id,
                level,
            ));
        }
        AbilityTaskOnFinished::ActivateAbility {
            source,
            target,
            handle,
        } => {
            if let Err(error) = execution_queue.push_chained_activation(
                source,
                target,
                handle,
                active_ability,
                active_context,
            ) {
                error!("failed to queue chained ability activation: {error}");
            }
        }
        AbilityTaskOnFinished::ApplyGameplayEffect {
            source,
            target,
            effect,
            level,
        } => {
            let payload = effect_payload_from_activation_context(source, level, active_context);
            execution_queue.push_application(target, effect, payload);
        }
        AbilityTaskOnFinished::ApplyGameplayEffectToTargets {
            source,
            fallback_target,
            effect,
            level,
        } => {
            let payload = effect_payload_from_activation_context(source, level, active_context);
            if let Some(target_data) = active_context.get_target_data() {
                for target in target_data.entities() {
                    execution_queue.push_application(target, effect.clone(), payload.clone());
                }
            } else {
                execution_queue.push_application(fallback_target, effect, payload);
            }
        }
    }
    AbilityTaskCompletion::Continue
}

fn effect_payload_from_activation_context(
    source: Entity,
    level: u32,
    activation_context: &AbilityActivationContext,
) -> EffectPayload {
    let payload = EffectPayload::new(source, activation_context.get_causer(), level)
        .with_instigator(activation_context.get_instigator());
    if let Some(source_snapshot) = activation_context.get_source_snapshot() {
        payload.with_source_snapshot(source_snapshot.clone())
    } else {
        payload
    }
}
