//! Ability task definitions, runtime state, completion events, and fixed-tick execution.

mod completion;
mod context;
mod definition;
mod event;
mod state;
mod ticking;

pub use context::AbilityTaskExecutionContext;
pub use definition::{AbilityTaskDef, AbilityTaskOnFinishedDef};
pub use event::AbilityTaskEvent;
pub use state::{AbilityTask, AbilityTaskKind, AbilityTaskOnFinished};
pub use ticking::tick_ability_tasks_system;
