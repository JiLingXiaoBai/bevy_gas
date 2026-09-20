//! Ability cost and cooldown validation, planning, and synchronous commit.
//!
//! Attribute affordability depends only on effect access. Planning owns no payment receipts;
//! execution temporarily pays external costs and compensates them if effect execution fails.

mod additional_cost;
mod affordability;
mod error;
mod execution;
mod planning;

pub use additional_cost::{AdditionalCostContext, AdditionalCostProvider};
pub(super) use affordability::check_ability_cost;
pub use error::AbilityCommitError;
pub use execution::commit_ability;
pub(super) use execution::execute_ability_commit_plans;
pub(super) use planning::prepare_ability_commit_plans;
