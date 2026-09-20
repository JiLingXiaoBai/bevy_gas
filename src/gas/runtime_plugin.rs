//! Plugin composition and ordered fixed-tick scheduling for the GAS runtime.
//!
//! Foundation plugins initialize shared resources, the runtime owns fixed-tick scheduling,
//! and the plugin group combines both for default or external-cost-enabled installation.

mod foundation;
mod group;
mod runtime;

pub use foundation::{GameplayTagPlugin, RandomPlugin, UniqueNamePlugin};
pub use group::GameplayAbilitySystemPlugin;
pub use runtime::{GameplayAbilitySystemRuntimePlugin, GameplayAbilitySystemSet};
