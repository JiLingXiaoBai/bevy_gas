use super::{
    AbilityActivationContext, AbilitySpecHandle, AbilityTaskExecutionContext,
    AbilityTaskOnFinished, ActiveAbilityHandle, effect_payload_from_ability_context,
};
use crate::gameplay_execution::GameplayExecutionQueue;
use crate::gameplay_targeting::AbilityActivationTargets;
use crate::unique_names::UniqueName;
use bevy::prelude::*;

/// Event emitted by an ability task completion action.
#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    context: AbilityTaskExecutionContext,
    targets: AbilityActivationTargets,
    active_ability: ActiveAbilityHandle,
    event_id: UniqueName,
}

impl AbilityTaskEvent {
    /// Creates an ability task event from the task's shared execution context.
    pub fn new(
        context: AbilityTaskExecutionContext,
        targets: AbilityActivationTargets,
        active_ability: ActiveAbilityHandle,
        event_id: UniqueName,
    ) -> Self {
        Self {
            context,
            targets,
            active_ability,
            event_id,
        }
    }

    /// Returns the task's shared execution context.
    pub fn get_context(&self) -> &AbilityTaskExecutionContext {
        &self.context
    }

    /// Returns the ability owner that started the task.
    pub fn get_source(&self) -> Entity {
        self.context.get_source()
    }

    /// Returns the complete target selection inherited from the ability activation.
    pub fn get_targets(&self) -> &AbilityActivationTargets {
        &self.targets
    }

    /// Returns the primary target inherited from the ability activation.
    pub fn get_target(&self) -> Entity {
        self.targets.get_primary_target()
    }

    /// Returns the active ability instance that owned the task.
    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    /// Returns the granted ability handle that owns the task.
    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.context.get_spec_handle()
    }

    /// Returns the event identifier supplied by the completion action.
    pub fn get_event_id(&self) -> UniqueName {
        self.event_id
    }

    /// Returns the captured ability level.
    pub fn get_level(&self) -> u32 {
        self.context.get_level()
    }
}

pub(crate) enum AbilityTaskCompletion {
    Continue,
    EndAbility,
}

pub(crate) fn dispatch_ability_task_completion(
    active_ability: ActiveAbilityHandle,
    context: AbilityTaskExecutionContext,
    on_finished: AbilityTaskOnFinished,
    targets: &AbilityActivationTargets,
    active_context: &AbilityActivationContext,
    commands: &mut Commands,
    execution_queue: &mut GameplayExecutionQueue,
) -> AbilityTaskCompletion {
    match on_finished {
        AbilityTaskOnFinished::None => {}
        AbilityTaskOnFinished::EndAbility => return AbilityTaskCompletion::EndAbility,
        AbilityTaskOnFinished::Batch { actions } => {
            for action in actions {
                let completion = dispatch_ability_task_completion(
                    active_ability,
                    context,
                    action,
                    targets,
                    active_context,
                    commands,
                    execution_queue,
                );
                if matches!(completion, AbilityTaskCompletion::EndAbility) {
                    return completion;
                }
            }
        }
        AbilityTaskOnFinished::EmitEvent { event_id } => {
            commands.trigger(AbilityTaskEvent::new(
                context,
                targets.clone(),
                active_ability,
                event_id,
            ));
        }
        AbilityTaskOnFinished::ActivateAbility { handle } => {
            if let Err(error) = execution_queue.push_chained_activation(
                context.get_source(),
                targets.clone(),
                handle,
                active_ability,
                active_context,
            ) {
                error!("failed to queue chained ability activation: {error}");
            }
        }
        AbilityTaskOnFinished::ApplyGameplayEffect { effect } => {
            let payload = effect_payload_from_ability_context(
                context.get_source(),
                context.get_level(),
                Some(active_context),
            );
            execution_queue.push_application(targets.get_primary_target(), effect, payload);
        }
        AbilityTaskOnFinished::ApplyGameplayEffectToTargets { effect } => {
            let payload = effect_payload_from_ability_context(
                context.get_source(),
                context.get_level(),
                Some(active_context),
            );
            for target in targets.entities() {
                execution_queue.push_application(target, effect.clone(), payload.clone());
            }
        }
    }
    AbilityTaskCompletion::Continue
}
