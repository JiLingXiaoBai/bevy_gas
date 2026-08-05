use super::{
    TargetingContinuation, TargetingDefinition, TargetingInput, TargetingRequest,
    TargetingRequestId,
};
use bevy::prelude::{Entity, Resource};
use std::collections::VecDeque;
use std::sync::Arc;

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

    pub(super) fn pop(&mut self) -> Option<TargetingRequest> {
        self.requests.pop_front()
    }
}
