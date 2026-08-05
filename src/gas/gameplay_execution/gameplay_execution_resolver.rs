use super::{GameplayExecutionQueue, GameplayExecutionRequest};
use crate::ability_system::{AbilitySystemParams, execute_ability_activation_in_batch};
use crate::gameplay_effects::{
    apply_gameplay_effect_in_batch, resolve_active_effect_tag_requirements_if_dirty,
};
use bevy::prelude::*;

pub(crate) fn drain_gameplay_execution_queue(
    execution_queue: &mut GameplayExecutionQueue,
    params: &mut AbilitySystemParams,
) {
    params
        .pending_active_abilities
        .retain_unapplied(&params.active_ability_query);
    resolve_active_effect_tag_requirements_if_dirty(params);

    while let Some(request) = execution_queue.pop() {
        match request {
            GameplayExecutionRequest::ActivateAbility(request) => {
                if let Err(error) = execute_ability_activation_in_batch(
                    request.get_source(),
                    request.get_target(),
                    request.get_handle(),
                    request.get_context().clone(),
                    execution_queue,
                    params,
                ) {
                    if error.is_rejection() {
                        debug!("queued ability activation was rejected: {error}");
                    } else {
                        error!("queued ability activation failed: {error}");
                    }
                }
            }
            GameplayExecutionRequest::ApplyGameplayEffect(request) => {
                if let Err(error) = apply_gameplay_effect_in_batch(
                    request.get_target(),
                    request.get_effect(),
                    params,
                    request.get_payload(),
                ) {
                    if error.is_rejection() {
                        debug!("queued gameplay effect was rejected: {error}");
                    } else {
                        error!("queued gameplay effect application failed: {error}");
                    }
                }
            }
        }

        resolve_active_effect_tag_requirements_if_dirty(params);
    }
}

/// Drains gameplay mutations in strict cross-type FIFO order.
pub fn process_gameplay_execution_queue_system(
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    mut params: AbilitySystemParams,
) {
    drain_gameplay_execution_queue(&mut execution_queue, &mut params);
}

/// Returns whether the gameplay mutation FIFO contains work.
pub fn gameplay_execution_queue_has_work(queue: Option<Res<GameplayExecutionQueue>>) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}
