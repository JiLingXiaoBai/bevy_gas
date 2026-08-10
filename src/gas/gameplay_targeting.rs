//! Validated, deterministic target acquisition pipelines and queued requests.

mod ability_target_data;
mod acquisition;
mod activation_targets;
mod targeting_definition;
mod targeting_queue;

pub use ability_target_data::{AbilityTargetData, AbilityTargetHit};
pub use acquisition::{TargetingCandidateQuery, TargetingError, acquire_targets};
pub use activation_targets::{AbilityActivationTargets, AbilityActivationTargetsError};
pub use targeting_definition::{
    Targetable, TargetingDefinition, TargetingDefinitionError, TargetingOperation,
    TargetingSortOrder,
};
pub use targeting_queue::{
    TargetingContinuation, TargetingInput, TargetingRequestId, TargetingRequestQueue,
    TargetingResultEvent, process_targeting_request_queue_system, targeting_request_queue_has_work,
};
