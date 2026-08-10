use super::super::component::AbilitySystemComponent;
use super::super::params::{AbilitySystemParams, PendingActiveGameplayAbilities};
use crate::gameplay_abilities::{
    AbilityActivationStatus, AbilityTaskCompletion, AbilityTaskDef, AbilityTaskExecutionContext,
    ActiveAbilityHandle, ActiveGameplayAbility, dispatch_ability_task_completion,
};
use crate::gameplay_execution::{AbilityActivationRequest, GameplayExecutionQueue};
use crate::gameplay_tags::{GameplayTagError, GameplayTagManager};
use bevy::prelude::*;

impl AbilitySystemComponent {
    pub(super) fn start_ability(
        &mut self,
        request: &AbilityActivationRequest,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
        pending_active_abilities: &mut PendingActiveGameplayAbilities,
    ) -> Result<ActiveAbilityHandle, GameplayTagError> {
        let blocked_tags = self
            .find_ability_spec(request.get_handle())
            .map(|spec| {
                spec.get_ability()
                    .get_tags()
                    .get_block_abilities_with_tags()
                    .to_vec()
            })
            .unwrap_or_default();
        self.blocked_ability_tags_mut()
            .add_tags(&blocked_tags, tag_manager)?;

        if let Some(spec) = self.find_ability_spec_mut(request.get_handle()) {
            spec.increment_active_count();
        }

        let active_ability = ActiveGameplayAbility::new(
            request.get_source(),
            request.get_handle(),
            request.get_target(),
            AbilityActivationStatus::Active,
            request.get_context().clone(),
        );
        let mut entity_commands = commands.spawn(active_ability.clone());
        let active_handle = entity_commands.id();
        entity_commands.set_parent_in_place(request.get_source());
        pending_active_abilities.insert(active_handle, active_ability);

        Ok(active_handle)
    }
}

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
        context.request.get_target(),
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
    ends_ability
}
