//! Runtime catalog and owner-local bindings for configured abilities.

use super::ConfigError;

mod definitions;
mod grants;

pub use definitions::{AbilityId, CompiledAbility, EffectId, GameplayCatalog};
pub use grants::{ConfiguredAbilities, grant_ability, revoke_ability};
