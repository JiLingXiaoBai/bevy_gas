use super::super::commit::AdditionalCostProvider;
use super::super::lifecycle::{finish_ability_startup, finish_ability_with_status};
use super::super::params::AbilitySystemParams;
use super::execution::begin_ability_activation;
use crate::gameplay_abilities::{
    AbilityActivationStatus, AbilityTaskDef, AbilityTaskEvent, AbilityTaskExecutionContext,
    AbilityTaskOnFinished, ActiveAbilityHandle, GameplayAbility,
    effect_payload_from_ability_context,
};
use crate::gameplay_effects::{
    GameplayEffect, apply_gameplay_effect_in_batch, resolve_active_effect_tag_requirements_if_dirty,
};
use crate::gameplay_execution::AbilityActivationRequest;
use bevy::prelude::*;
use std::sync::Arc;

/// Owns a suspended activation so child startup can finish without recursive activation calls.
pub(super) struct StartupAbility {
    ability: Arc<GameplayAbility>,
    request: AbilityActivationRequest,
    active_handle: ActiveAbilityHandle,
    task_context: AbilityTaskExecutionContext,
    next_task: usize,
    pending_actions: Vec<AbilityTaskOnFinished>,
}

impl StartupAbility {
    pub(super) fn new(
        ability: Arc<GameplayAbility>,
        request: AbilityActivationRequest,
        active_handle: ActiveAbilityHandle,
        level: u32,
    ) -> Self {
        let task_context =
            AbilityTaskExecutionContext::new(request.get_source(), request.get_handle(), level);
        Self {
            ability,
            request,
            active_handle,
            task_context,
            next_task: 0,
            pending_actions: Vec::new(),
        }
    }

    /// Runs until a child activation must complete or this startup has finished.
    fn advance(
        &mut self,
        params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
    ) -> Option<AbilityActivationRequest> {
        // A synchronous child may cancel this parent before its commands become visible.
        if !self.is_active(params) {
            self.finish(false, params);
            return None;
        }

        loop {
            if let Some(action) = self.pending_actions.pop() {
                match action {
                    AbilityTaskOnFinished::None => {}
                    AbilityTaskOnFinished::EndAbility => {
                        self.finish(true, params);
                        return None;
                    }
                    AbilityTaskOnFinished::Batch { actions } => {
                        self.pending_actions.extend(actions.into_iter().rev());
                    }
                    AbilityTaskOnFinished::EmitEvent { event_id } => {
                        params.commands.trigger(AbilityTaskEvent::new(
                            self.task_context,
                            self.request.get_targets().clone(),
                            self.active_handle,
                            event_id,
                        ));
                    }
                    AbilityTaskOnFinished::ApplyGameplayEffect { effect } => {
                        apply_startup_effect(
                            self.request.get_target(),
                            &effect,
                            &self.request,
                            self.task_context.get_level(),
                            params,
                        );
                    }
                    AbilityTaskOnFinished::ApplyGameplayEffectToTargets { effect } => {
                        for target in self.request.get_targets().entities() {
                            apply_startup_effect(
                                target,
                                &effect,
                                &self.request,
                                self.task_context.get_level(),
                                params,
                            );
                        }
                    }
                    AbilityTaskOnFinished::ActivateAbility { handle } => {
                        match self
                            .request
                            .get_context()
                            .child_for_chained_ability(self.active_handle, handle)
                        {
                            Ok(context) => {
                                return Some(AbilityActivationRequest::new(
                                    self.request.get_source(),
                                    self.request.get_targets().clone(),
                                    handle,
                                    context,
                                ));
                            }
                            Err(error) => error!("invalid startup ability chain: {error}"),
                        }
                    }
                }
                continue;
            }

            let Some(task_def) = self.ability.get_startup_tasks().get(self.next_task) else {
                self.finish(false, params);
                return None;
            };
            self.next_task += 1;
            match task_def {
                AbilityTaskDef::Instant { on_finished } => {
                    self.pending_actions.push(on_finished.instantiate());
                }
                AbilityTaskDef::WaitTicks { .. } => {
                    params
                        .commands
                        .spawn(task_def.instantiate(self.active_handle, self.task_context))
                        .set_parent_in_place(self.active_handle);
                }
            }
        }
    }

    fn is_active(&self, params: &AbilitySystemParams<'_, '_, impl AdditionalCostProvider>) -> bool {
        params
            .pending_active_abilities
            .iter()
            .find_map(|(handle, active)| {
                (handle == self.active_handle).then_some(active.get_status())
            })
            .or_else(|| {
                params
                    .active_ability_query
                    .get(self.active_handle)
                    .ok()
                    .map(|(_, active)| active.get_status())
            })
            .is_some_and(|status| status == AbilityActivationStatus::Active)
    }

    fn finish(
        &self,
        ends_ability: bool,
        params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
    ) {
        finish_ability_startup(
            self.request.get_source(),
            self.active_handle,
            &mut params.commands,
        );
        if ends_ability {
            finish_ability_with_status(
                self.request.get_source(),
                self.active_handle,
                AbilityActivationStatus::Ending,
                params,
            );
        }
    }
}

pub(super) fn run_startup_ability_tasks(
    mut current: StartupAbility,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) {
    // Only chained activations allocate a parent stack; ordinary startup stays in this frame.
    let mut parents = Vec::new();
    loop {
        if let Some(request) = current.advance(params) {
            let result = begin_ability_activation(request, params);
            resolve_active_effect_tag_requirements_if_dirty(&mut params.effects);
            match result {
                Ok(child) => {
                    parents.push(current);
                    current = child;
                }
                Err(error) => {
                    if error.is_rejection() {
                        debug!("startup ability activation was rejected: {error}");
                    } else {
                        error!("startup ability activation failed: {error}");
                    }
                }
            }
        } else if let Some(parent) = parents.pop() {
            current = parent;
        } else {
            break;
        }
    }
}

/// Applies one startup effect before the next action observes tags or attributes.
fn apply_startup_effect(
    target: Entity,
    effect: &Arc<GameplayEffect>,
    request: &AbilityActivationRequest,
    level: u32,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) {
    let payload = effect_payload_from_ability_context(
        request.get_source(),
        level,
        Some(request.get_context()),
    );
    if let Err(error) =
        apply_gameplay_effect_in_batch(target, effect, &mut params.effects, &payload)
    {
        if error.is_rejection() {
            debug!("ability startup effect was rejected: {error}");
        } else {
            error!("ability startup effect failed: {error}");
        }
    }
    resolve_active_effect_tag_requirements_if_dirty(&mut params.effects);
}
