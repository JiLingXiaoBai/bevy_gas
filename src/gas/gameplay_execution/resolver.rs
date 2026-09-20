use super::{
    GameplayExecutionError, GameplayExecutionOutcome, GameplayExecutionQueue,
    GameplayExecutionRequest, GameplayExecutionResult,
};
use crate::ability_system::{
    AbilitySystemParams, AdditionalCostProvider, execute_ability_activation_in_batch,
};
use crate::gameplay_effects::{
    apply_gameplay_effect_in_batch, resolve_active_effect_tag_requirements_if_dirty,
};
use bevy::prelude::*;

fn drain_gameplay_execution_queue(
    execution_queue: &mut GameplayExecutionQueue,
    params: &mut AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
    mut on_completed: impl FnMut(GameplayExecutionResult),
) {
    params
        .pending_active_abilities
        .retain_unapplied(&params.active_ability_query);
    resolve_active_effect_tag_requirements_if_dirty(params);

    while let Some((request_id, request)) = execution_queue.pop() {
        let (source, target) = match &request {
            GameplayExecutionRequest::ActivateAbility(request) => {
                (request.get_source(), request.get_target())
            }
            GameplayExecutionRequest::ApplyGameplayEffect(request) => {
                (request.get_payload().get_source(), request.get_target())
            }
        };
        let result = match request {
            GameplayExecutionRequest::ActivateAbility(request) => {
                execute_ability_activation_in_batch(request, params)
                    .map(|_| ())
                    .map_err(GameplayExecutionError::AbilityActivation)
            }
            GameplayExecutionRequest::ApplyGameplayEffect(request) => {
                apply_gameplay_effect_in_batch(
                    request.get_target(),
                    request.get_effect(),
                    params,
                    request.get_payload(),
                )
                .map(|_| ())
                .map_err(GameplayExecutionError::EffectApplication)
            }
        };
        if let Err(error) = &result {
            if error.is_rejection() {
                debug!("queued gameplay request was rejected: {error}");
            } else {
                error!("queued gameplay request failed: {error}");
            }
        }
        resolve_active_effect_tag_requirements_if_dirty(params);
        on_completed(GameplayExecutionResult {
            request_id,
            source,
            target,
            outcome: GameplayExecutionOutcome::from_result(result),
        });
    }
}

/// Drains gameplay mutations in FIFO order and publishes one result per consumed request.
///
/// Startup Instant effects and chained activations finish inside the owning activation and do
/// not enqueue requests or publish independent results. Task events remain deferred.
/// Result consumers run after this system. Any requests they enqueue run in the next fixed tick.
pub fn process_gameplay_execution_queue_system(
    execution_queue: ResMut<GameplayExecutionQueue>,
    params: AbilitySystemParams,
    results: MessageWriter<GameplayExecutionResult>,
) {
    process_gameplay_execution_queue_with_costs_system::<()>(execution_queue, params, results);
}

/// Returns whether the gameplay mutation FIFO contains work.
pub fn gameplay_execution_queue_has_work(queue: Option<Res<GameplayExecutionQueue>>) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}

/// Drains the gameplay FIFO using external-cost provider `P` and publishes request results.
///
/// Install through `GameplayAbilitySystemRuntimePlugin::with_additional_costs` to ensure only
/// one resolver is registered. This preserves the default FIFO and synchronous startup ordering.
pub fn process_gameplay_execution_queue_with_costs_system<P: AdditionalCostProvider>(
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    mut params: AbilitySystemParams<P>,
    mut results: MessageWriter<GameplayExecutionResult>,
) {
    drain_gameplay_execution_queue(&mut execution_queue, &mut params, |result| {
        results.write(result);
    });
}
