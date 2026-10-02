//! Ability instance bookkeeping, state transitions, and ECS lifetime cleanup.

mod cleanup;
mod instances;
mod transitions;

pub use cleanup::cleanup_finished_abilities_system;
pub(crate) use cleanup::{cleanup_discarded_ability_system, cleanup_discarded_active_ability};
pub use transitions::{cancel_ability, end_ability};
pub(super) use transitions::{cancel_active_abilities_with_tags, finish_ability_with_status};

pub(super) use instances::finish_ability_startup;
