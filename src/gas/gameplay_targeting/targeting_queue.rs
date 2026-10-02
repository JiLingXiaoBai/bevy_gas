//! Queued target acquisition, captured request data, and continuation dispatch.

mod processing;
mod queue;
mod request;

pub use processing::{process_targeting_request_queue_system, targeting_request_queue_has_work};
pub use queue::TargetingRequestQueue;
pub use request::{
    TargetingContinuation, TargetingInput, TargetingRequestId, TargetingResultEvent,
};
