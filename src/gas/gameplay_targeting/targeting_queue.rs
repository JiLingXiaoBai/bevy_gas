use super::{
    AbilityTargetData, TargetingCandidateQuery, TargetingDefinition, TargetingError,
    acquire_targets,
};
use crate::ability_system::AbilityActivationQueue;
use crate::gameplay_abilities::{AbilityActivationContext, AbilitySpecHandle};
use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;

/// Stable identifier assigned to one queued targeting request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetingRequestId(u64);

impl TargetingRequestId {
    /// Returns the numeric request identifier.
    pub fn get_value(self) -> u64 {
        self.0
    }
}

/// Spatial input captured when a targeting request is submitted.
#[derive(Debug, Clone, Copy)]
pub struct TargetingInput {
    origin: Vec3,
    direction: Vec3,
    explicit_target: Option<Entity>,
}

impl TargetingInput {
    /// Creates targeting input from a world-space origin and direction.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction,
            explicit_target: None,
        }
    }

    /// Sets the entity consumed by `SelectExplicitEntity`.
    pub fn with_explicit_target(mut self, target: Entity) -> Self {
        self.explicit_target = Some(target);
        self
    }

    /// Returns the captured world-space origin.
    pub fn get_origin(self) -> Vec3 {
        self.origin
    }

    /// Returns the captured world-space direction.
    pub fn get_direction(self) -> Vec3 {
        self.direction
    }

    /// Returns the optional explicitly selected entity.
    pub fn get_explicit_target(self) -> Option<Entity> {
        self.explicit_target
    }
}

/// Describes what should happen after a targeting request succeeds.
#[derive(Clone)]
pub enum TargetingContinuation {
    /// Only emits a [`TargetingResultEvent`].
    EmitResult,
    /// Adds an ability activation carrying the acquired target data to the queue.
    ActivateAbility {
        handle: AbilitySpecHandle,
        context: Box<AbilityActivationContext>,
    },
}

impl TargetingContinuation {
    /// Creates an ability-activation continuation.
    pub fn activate_ability(handle: AbilitySpecHandle, context: AbilityActivationContext) -> Self {
        Self::ActivateAbility {
            handle,
            context: Box::new(context),
        }
    }
}

struct TargetingRequest {
    id: TargetingRequestId,
    source: Entity,
    input: TargetingInput,
    definition: Arc<TargetingDefinition>,
    continuation: TargetingContinuation,
}

/// Triggered after a queued targeting request succeeds or fails.
#[derive(Event, Clone)]
pub struct TargetingResultEvent {
    request_id: TargetingRequestId,
    source: Entity,
    result: Result<AbilityTargetData, TargetingError>,
}

impl TargetingResultEvent {
    /// Returns the completed request identifier.
    pub fn get_request_id(&self) -> TargetingRequestId {
        self.request_id
    }

    /// Returns the entity that submitted the targeting request.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the acquired data or targeting failure.
    pub fn get_result(&self) -> &Result<AbilityTargetData, TargetingError> {
        &self.result
    }
}

/// FIFO queue for targeting work in `FixedUpdate`.
#[derive(Resource)]
pub struct TargetingRequestQueue {
    requests: VecDeque<TargetingRequest>,
    next_request_id: u64,
}

impl Default for TargetingRequestQueue {
    fn default() -> Self {
        Self {
            requests: VecDeque::new(),
            next_request_id: 1,
        }
    }
}

impl TargetingRequestQueue {
    /// Queues one targeting request and returns its stable identifier.
    pub fn push_request(
        &mut self,
        source: Entity,
        input: TargetingInput,
        definition: Arc<TargetingDefinition>,
        continuation: TargetingContinuation,
    ) -> TargetingRequestId {
        let id = TargetingRequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        self.requests.push_back(TargetingRequest {
            id,
            source,
            input,
            definition,
            continuation,
        });
        id
    }

    /// Returns whether the queue has no pending requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Returns the number of pending requests.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Removes every pending request.
    pub fn clear(&mut self) {
        self.requests.clear();
    }

    fn pop(&mut self) -> Option<TargetingRequest> {
        self.requests.pop_front()
    }
}

/// Processes queued targeting requests and dispatches their continuations.
pub fn process_targeting_request_queue_system(
    mut commands: Commands,
    mut targeting_queue: ResMut<TargetingRequestQueue>,
    mut activation_queue: ResMut<AbilityActivationQueue>,
    query: TargetingCandidateQuery,
) {
    while let Some(request) = targeting_queue.pop() {
        let result = acquire_targets(request.source, request.input, &request.definition, &query);

        if let Ok(target_data) = &result
            && let TargetingContinuation::ActivateAbility { handle, context } = request.continuation
        {
            let target = target_data.primary_entity().unwrap_or(request.source);
            activation_queue.push_activation(
                request.source,
                target,
                handle,
                context.with_target_data(target_data.clone()),
            );
        }

        commands.trigger(TargetingResultEvent {
            request_id: request.id,
            source: request.source,
            result,
        });
    }
}

/// Returns whether the targeting queue contains work.
pub fn targeting_request_queue_has_work(queue: Option<Res<TargetingRequestQueue>>) -> bool {
    queue.is_some_and(|queue| !queue.is_empty())
}
