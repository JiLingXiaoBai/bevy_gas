use super::{
    AbilityActivationTargets, AbilityTargetData, TargetingCandidateQuery, TargetingDefinition,
    TargetingError, acquire_targets,
};
use crate::gameplay_abilities::{AbilityActivationContext, AbilitySpecHandle};
use crate::gameplay_execution::GameplayExecutionQueue;

mod processing;
mod queue;
mod request;

use request::TargetingRequest;

pub use processing::{process_targeting_request_queue_system, targeting_request_queue_has_work};
pub use queue::TargetingRequestQueue;
pub use request::{
    TargetingContinuation, TargetingInput, TargetingRequestId, TargetingResultEvent,
};
