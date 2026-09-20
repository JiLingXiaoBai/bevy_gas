//! Deterministic cross-type gameplay request queue and resolver.
//!
//! Ability activation and effect application requests share one FIFO so their
//! relative ordering remains observable within a fixed tick.
//! Startup Instant actions resolve inside their activation before the next queued request.

mod queue;
mod request;
mod resolver;
mod result;

pub use queue::{GameplayExecutionQueue, GameplayExecutionQueueError, GameplayExecutionRequestId};
pub use request::{
    AbilityActivationRequest, GameplayEffectApplicationRequest, GameplayExecutionRequest,
};
pub use resolver::{
    gameplay_execution_queue_has_work, process_gameplay_execution_queue_system,
    process_gameplay_execution_queue_with_costs_system,
};

pub use result::{GameplayExecutionError, GameplayExecutionOutcome, GameplayExecutionResult};
