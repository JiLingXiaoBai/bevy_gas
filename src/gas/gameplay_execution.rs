//! Deterministic cross-type gameplay request queue and resolver.
//!
//! Ability activation and effect application requests share one FIFO so their
//! relative ordering remains observable within a fixed tick.

mod queue;
mod request;
mod resolver;

pub use queue::GameplayExecutionQueue;
pub use request::{
    AbilityActivationRequest, GameplayEffectApplicationRequest, GameplayExecutionRequest,
};
pub(crate) use resolver::drain_gameplay_execution_queue;
pub use resolver::{gameplay_execution_queue_has_work, process_gameplay_execution_queue_system};
