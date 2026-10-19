//! Target data contracts shared by ability activation, tasks, and external game adapters.
//!
//! GAS consumes captured targets without owning their selection, spatial index, or collision
//! detection. Games construct these values and submit activation or effect requests explicitly.

mod ability_target_data;
mod activation_targets;
mod error;

pub use ability_target_data::{AbilityTargetData, AbilityTargetHit};
pub use activation_targets::AbilityActivationTargets;
pub use error::AbilityActivationTargetsError;
