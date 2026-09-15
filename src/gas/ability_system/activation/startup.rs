use super::super::lifecycle::finish_ability_startup;
use super::super::params::AbilitySystemParams;
use crate::gameplay_abilities::{
    AbilityTaskCompletion, AbilityTaskDef, AbilityTaskExecutionContext, ActiveAbilityHandle,
    dispatch_ability_task_completion,
};
use crate::gameplay_execution::{AbilityActivationRequest, GameplayExecutionQueue};
use bevy::prelude::*;

/// Borrows the canonical activation request needed to start sibling ability tasks.
pub(super) struct StartupAbilityTaskContext<'a> {
    pub(super) active_handle: ActiveAbilityHandle,
    pub(super) request: &'a AbilityActivationRequest,
    pub(super) level: u32,
}

pub(super) fn start_startup_ability_tasks(
    startup_tasks: &[AbilityTaskDef],
    context: StartupAbilityTaskContext,
    execution_queue: &mut GameplayExecutionQueue,
    params: &mut AbilitySystemParams,
) -> bool {
    let task_context = AbilityTaskExecutionContext::new(
        context.request.get_source(),
        context.request.get_handle(),
        context.level,
    );
    let mut ends_ability = false;
    for task_def in startup_tasks {
        match task_def {
            AbilityTaskDef::Instant { on_finished } => {
                let completion = dispatch_ability_task_completion(
                    context.active_handle,
                    task_context,
                    on_finished.instantiate(),
                    context.request.get_targets(),
                    context.request.get_context(),
                    &mut params.commands,
                    execution_queue,
                );
                ends_ability |= matches!(completion, AbilityTaskCompletion::EndAbility);
                if ends_ability {
                    break;
                }
            }
            AbilityTaskDef::WaitTicks { .. } => {
                let mut task_commands = params
                    .commands
                    .spawn(task_def.instantiate(context.active_handle, task_context));
                task_commands.set_parent_in_place(context.active_handle);
            }
        }
    }
    finish_ability_startup(
        context.request.get_source(),
        context.active_handle,
        &mut params.commands,
    );
    ends_ability
}
