use super::{
    AbilityActivationTargets, GameplayExecutionQueue, TargetingCandidateQuery,
    TargetingContinuation, TargetingRequestQueue, TargetingResultEvent, acquire_targets,
};
use bevy::prelude::*;

/// Processes queued targeting requests and dispatches their continuations.
pub fn process_targeting_request_queue_system(
    mut commands: Commands,
    mut targeting_queue: ResMut<TargetingRequestQueue>,
    mut execution_queue: ResMut<GameplayExecutionQueue>,
    query: TargetingCandidateQuery,
) {
    while let Some(request) = targeting_queue.pop() {
        let result = acquire_targets(request.source, request.input, &request.definition, &query);

        if let Ok(target_data) = &result
            && let TargetingContinuation::ActivateAbility { handle, context } = request.continuation
        {
            match AbilityActivationTargets::acquired(target_data.clone()) {
                Ok(targets) => {
                    execution_queue.push_activation(request.source, targets, handle, *context);
                }
                Err(error) => {
                    error!("targeting produced invalid ability activation targets: {error}");
                }
            }
        }

        commands.trigger(TargetingResultEvent::new(
            request.id,
            request.source,
            result,
        ));
    }
}

/// Returns whether the targeting queue contains work.
pub fn targeting_request_queue_has_work(queue: Option<Res<TargetingRequestQueue>>) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}
