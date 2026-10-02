use super::super::{
    AbilityActivationContext, ActiveAbilityHandle, effect_payload_from_ability_context,
};
use super::{AbilityTaskEvent, AbilityTaskExecutionContext, AbilityTaskOnFinished};
use crate::gameplay_execution::GameplayExecutionQueue;
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::*;

pub(super) enum AbilityTaskCompletion {
    Continue,
    EndAbility,
}

pub(super) fn dispatch_ability_task_completion(
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
            if let Err(error) =
                execution_queue.push_application(targets.get_primary_target(), effect, payload)
            {
                error!("failed to queue ability-task effect: {error}");
            }
        }
        AbilityTaskOnFinished::ApplyGameplayEffectToTargets { effect } => {
            let payload = effect_payload_from_ability_context(
                context.get_source(),
                context.get_level(),
                Some(active_context),
            );
            for target in targets.entities() {
                if let Err(error) =
                    execution_queue.push_application(target, effect.clone(), payload.clone())
                {
                    error!("failed to queue ability-task effect: {error}");
                }
            }
        }
    }
    AbilityTaskCompletion::Continue
}
