use super::super::commit::{
    AbilityCommitError, AdditionalCostContext, AdditionalCostProvider, check_ability_cost,
};
use super::super::component::AbilitySystemComponent;
use super::super::params::{AbilityActivationCheckParams, AbilitySystemParams};
use crate::gameplay_abilities::GameplayAbility;
use crate::gameplay_tags::GameplayTagContainer;
use bevy::prelude::Entity;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Reason a read-only activation precheck found an ability unavailable.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityActivationCheckError {
    /// Another active ability blocks this definition's asset tags.
    BlockedByAbility,
    /// The owner has a tag that blocks activation.
    ActivationBlocked,
    /// The owner's tag container does not satisfy the required activation tags.
    MissingRequiredTags,
    /// The owner currently has a granted cooldown tag.
    CooldownActive,
    /// Cost evaluation or affordability checking failed.
    Cost(AbilityCommitError),
}

impl fmt::Display for AbilityActivationCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockedByAbility => write!(f, "another active ability blocks activation"),
            Self::ActivationBlocked => write!(f, "an owned gameplay tag blocks activation"),
            Self::MissingRequiredTags => write!(f, "required activation tags are missing"),
            Self::CooldownActive => write!(f, "the ability is on cooldown"),
            Self::Cost(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for AbilityActivationCheckError {}

/// Inspects activation tags, additive cost, and external-cost affordability without mutation.
///
/// This does not validate a granted handle, active-instance limits, instant cost duration, or
/// effect application requirements, and never rolls probability. Success does not guarantee that
/// a subsequent activation succeeds. Missing optional tag containers retain the runtime's existing
/// tag-check semantics. Cost previews exclude sources selected by the cost's removal tags and do
/// not invoke post-execute callbacks. External previews use `P::ReadOnly`; use the same provider
/// type as the runtime plugin. Preview context has no captured activation metadata.
///
/// # Parameters
///
/// - `source`: Owner whose activation tags and current attributes are checked.
/// - `target`: Modifier-evaluation target for this preview; actual commit evaluates against `source`.
/// - `ability`: Shared ability definition to inspect.
/// - `level`: Ability level used to evaluate cost magnitudes.
/// - `params`: Read-only ECS access for the current snapshot.
///
/// # Returns
///
/// `Ok(())` if the inspected rules pass, or the first concrete unavailable reason.
pub fn can_activate_ability<P: AdditionalCostProvider>(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &AbilityActivationCheckParams<'_, '_, P>,
) -> Result<(), AbilityActivationCheckError> {
    check_activation_tags(
        params.asc_query.get(source).ok(),
        params.effects.tag_container_query.get(source).ok(),
        ability,
    )?;
    check_ability_cost(source, target, ability, level, &params.effects)
        .map_err(AbilityActivationCheckError::Cost)?;
    if !ability.get_additional_costs().is_empty() {
        P::check_readonly(
            &params.additional_costs,
            &AdditionalCostContext {
                source,
                level,
                activation: None,
            },
            ability.get_additional_costs(),
        )
        .map_err(AbilityCommitError::AdditionalCost)
        .map_err(AbilityActivationCheckError::Cost)?;
    }
    Ok(())
}

pub(super) fn passes_ability_activation_requirements(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    params: &AbilitySystemParams<'_, '_, impl AdditionalCostProvider>,
) -> bool {
    check_activation_tags(
        params.asc_query.get(source).ok(),
        params.effects.tag_container_query.get(source).ok(),
        ability,
    )
    .is_ok()
}

fn check_activation_tags(
    asc: Option<&AbilitySystemComponent>,
    tags: Option<&GameplayTagContainer>,
    ability: &GameplayAbility,
) -> Result<(), AbilityActivationCheckError> {
    if asc.is_some_and(|asc| {
        asc.get_blocked_ability_tags()
            .has_any(ability.get_tags().get_ability_asset_tags())
    }) {
        return Err(AbilityActivationCheckError::BlockedByAbility);
    }
    if let Some(tags) = tags {
        let ability_tags = ability.get_tags();
        if tags.has_any(ability_tags.get_activation_blocked_tags()) {
            return Err(AbilityActivationCheckError::ActivationBlocked);
        }
        if !tags.has_all(ability_tags.get_activation_required_tags()) {
            return Err(AbilityActivationCheckError::MissingRequiredTags);
        }
        if let Some(cooldown) = ability.get_cooldown()
            && tags.has_any(cooldown.get_tags().get_granted_tags())
        {
            return Err(AbilityActivationCheckError::CooldownActive);
        }
    }
    Ok(())
}
