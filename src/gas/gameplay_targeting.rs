//! Validated, deterministic target acquisition pipelines and queued requests.

mod ability_target_data;
mod acquisition;
mod activation_targets;
mod error;
mod runtime;
mod spatial_index;
mod targeting_definition;
mod targeting_queue;

pub use ability_target_data::{AbilityTargetData, AbilityTargetHit};
pub use acquisition::{TargetingCandidateQuery, TargetingSystemParams, acquire_targets};
pub use activation_targets::AbilityActivationTargets;
pub use error::{AbilityActivationTargetsError, TargetingDefinitionError, TargetingError};
pub(crate) use runtime::install_targeting;
pub use spatial_index::{TargetingSpatialIndex, sync_targeting_spatial_index_system};
pub use targeting_definition::{
    Targetable, TargetingDefinition, TargetingOperation, TargetingSortOrder,
};
pub use targeting_queue::{
    TargetingContinuation, TargetingInput, TargetingRequestId, TargetingRequestQueue,
    TargetingResultEvent, process_targeting_request_queue_system, targeting_request_queue_has_work,
};
