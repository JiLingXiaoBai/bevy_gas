use super::ConfigError;
use super::{AbilityId, GameplayCatalog};
use crate::{AbilitySpecHandle, AbilitySystemComponent};
use bevy::prelude::Component;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Maps stable configuration IDs to handles granted to one actor's ASC.
#[derive(Component, Default)]
pub struct ConfiguredAbilities {
    bindings: BTreeMap<AbilityId, AbilitySpecHandle>,
}

impl ConfiguredAbilities {
    /// Returns this owner's granted handle for `id`, or `None` if unbound.
    pub fn handle(&self, id: AbilityId) -> Option<AbilitySpecHandle> {
        self.bindings.get(&id).copied()
    }
}

/// Grants catalog ability `id` at `level` and records its owner-local handle.
///
/// `asc` and `bindings` must belong to the same entity. Returns the granted handle,
/// or an error for an unknown ability, unsupported level, or duplicate binding.
/// No grant or binding changes are made when validation fails.
pub fn grant_ability(
    asc: &mut AbilitySystemComponent,
    bindings: &mut ConfiguredAbilities,
    catalog: &GameplayCatalog,
    id: AbilityId,
    level: u32,
) -> Result<AbilitySpecHandle, ConfigError> {
    let ability = catalog
        .ability(id)
        .ok_or_else(|| ConfigError::new(format!("Ability[{}]", id.0), "unknown ability"))?;
    if !(1..=ability.max_level()).contains(&level) {
        return Err(ConfigError::new(
            format!("Ability[{}].level", id.0),
            format!("level {level} is outside 1..={}", ability.max_level()),
        ));
    }
    if bindings.bindings.contains_key(&id) {
        return Err(ConfigError::new(
            format!("Ability[{}]", id.0),
            "ability is already bound for this owner",
        ));
    }
    let handle = asc.give_ability(Arc::clone(ability.definition()), level);
    bindings.bindings.insert(id, handle);
    Ok(handle)
}

/// Revokes the ability bound to `id` from the same owner's ASC and bindings.
///
/// Returns `true` when a binding was removed, or `false` when it did not exist.
/// Active abilities return an error and keep their binding. A stale binding whose
/// ASC grant was already removed is cleared, allowing the ability to be granted again.
pub fn revoke_ability(
    asc: &mut AbilitySystemComponent,
    bindings: &mut ConfiguredAbilities,
    id: AbilityId,
) -> Result<bool, ConfigError> {
    let Some(handle) = bindings.handle(id) else {
        return Ok(false);
    };
    if asc.find_ability_spec(handle).is_some() && !asc.clear_ability(handle) {
        return Err(ConfigError::new(
            format!("Ability[{}]", id.0),
            "cannot revoke an active ability; end or cancel it first",
        ));
    }
    bindings.bindings.remove(&id);
    Ok(true)
}
