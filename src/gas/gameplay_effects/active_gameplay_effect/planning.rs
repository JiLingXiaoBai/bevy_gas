use super::super::gameplay_effect::{EffectPayload, GameplayEffect, StackOverflowPolicy};
use super::super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use super::super::{EffectContext, EffectSystemParams};
use super::application::{
    find_stackable_active_effect, is_blocked_by_application_immunity,
    passes_application_requirements,
};
use super::execution::validate_effect_execution_requirements;
use super::removal::collect_active_effects_with_tags_for_params;
use super::state::{ActiveEffectHandle, ActiveEffectStorageError};
use crate::attributes::{AttributeId, AttributeIdError, AttributeSetError};
use crate::gameplay_tags::GameplayTagError;
use crate::modifiers::ModifierSpec;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Prepared gameplay-effect work intended for immediate execution.
///
/// Execution revalidates structural ECS requirements, but it does not repeat
/// application-tag, immunity, probability, or stacking decisions captured here.
/// A plan is not a transactional or long-lived command.
pub struct GameplayEffectApplicationPlan {
    pub(super) source: Entity,
    pub(super) target: Entity,
    pub(super) spec: GameplayEffectSpec,
    pub(super) removed_effects: Vec<ActiveEffectHandle>,
    pub(super) kind: GameplayEffectApplicationKind,
}

pub(super) enum GameplayEffectApplicationKind {
    Instant,
    StackExisting {
        handle: ActiveEffectHandle,
        new_stack_count: u32,
    },
    CreateActive,
}

impl GameplayEffectApplicationPlan {
    /// Returns the prepared modifier specifications.
    pub fn get_modifier_specs(&self) -> &[ModifierSpec] {
        self.spec.get_modifier_specs()
    }

    /// Returns whether this plan applies an instant effect.
    pub fn is_instant(&self) -> bool {
        matches!(self.kind, GameplayEffectApplicationKind::Instant)
    }

    pub(super) fn changes_active_effect_requirements(&self) -> bool {
        !self.removed_effects.is_empty()
            || matches!(self.kind, GameplayEffectApplicationKind::CreateActive)
    }
}

/// Describes why a gameplay effect could not be prepared or executed.
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayEffectApplicationError {
    /// The configured application probability is not finite or outside `0.0..=1.0`.
    InvalidProbability { probability: f32 },
    /// The probability roll rejected the application.
    ProbabilityRejected,
    /// Source or target application tag requirements were not met.
    ApplicationRequirementsNotMet,
    /// An active immunity effect blocked the application.
    BlockedByImmunity,
    /// A duration effect resolved to zero ticks.
    InvalidDuration,
    /// The target cannot store active gameplay effects.
    MissingActiveGameplayEffects { target: Entity },
    /// The target has exhausted the representable active-effect slot space.
    ActiveEffectCapacityExceeded { target: Entity },
    /// The target does not have the attribute storage required by the effect.
    MissingAttributeSet { target: Entity },
    /// The target has attribute storage but has not initialized a modified attribute.
    MissingAttribute { target: Entity, id: AttributeId },
    /// The target does not have the tag container required by the effect.
    MissingTagContainer { target: Entity },
    /// The stacking policy rejected an application beyond its limit.
    StackOverflowRejected,
    /// A gameplay tag did not belong to the active tag manager.
    GameplayTag(GameplayTagError),
    /// An attribute ID did not belong to the active attribute manager.
    AttributeId(AttributeIdError),
}

impl fmt::Display for GameplayEffectApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProbability { probability } => write!(
                f,
                "gameplay effect probability must be finite and within 0.0..=1.0, got {probability}"
            ),
            Self::ProbabilityRejected => {
                write!(f, "gameplay effect probability roll rejected application")
            }
            Self::ApplicationRequirementsNotMet => write!(
                f,
                "gameplay effect application tag requirements were not met"
            ),
            Self::BlockedByImmunity => {
                write!(f, "gameplay effect application was blocked by immunity")
            }
            Self::InvalidDuration => {
                write!(
                    f,
                    "gameplay effect duration must be greater than zero ticks"
                )
            }
            Self::MissingActiveGameplayEffects { target } => write!(
                f,
                "gameplay effect target {target:?} has no ActiveGameplayEffects"
            ),
            Self::ActiveEffectCapacityExceeded { target } => write!(
                f,
                "gameplay effect target {target:?} exhausted active-effect handle capacity"
            ),
            Self::MissingAttributeSet { target } => {
                write!(f, "gameplay effect target {target:?} has no AttributeSet")
            }
            Self::MissingAttribute { target, id } => write!(
                f,
                "gameplay effect target {target:?} has not initialized attribute {}",
                id.to_index()
            ),
            Self::MissingTagContainer { target } => write!(
                f,
                "gameplay effect target {target:?} has no GameplayTagContainer"
            ),
            Self::StackOverflowRejected => {
                write!(f, "gameplay effect stacking policy rejected overflow")
            }
            Self::GameplayTag(error) => {
                write!(f, "gameplay effect contains an invalid tag: {error}")
            }
            Self::AttributeId(error) => write!(
                f,
                "gameplay effect contains an invalid attribute ID: {error}"
            ),
        }
    }
}

impl Error for GameplayEffectApplicationError {}

impl GameplayEffectApplicationError {
    /// Returns whether this error represents an expected gameplay rejection.
    pub const fn is_rejection(&self) -> bool {
        matches!(
            self,
            Self::ProbabilityRejected
                | Self::ApplicationRequirementsNotMet
                | Self::BlockedByImmunity
                | Self::StackOverflowRejected
        )
    }
}

impl From<GameplayTagError> for GameplayEffectApplicationError {
    fn from(value: GameplayTagError) -> Self {
        Self::GameplayTag(value)
    }
}

impl From<AttributeIdError> for GameplayEffectApplicationError {
    fn from(value: AttributeIdError) -> Self {
        Self::AttributeId(value)
    }
}

impl From<ActiveEffectStorageError> for GameplayEffectApplicationError {
    fn from(value: ActiveEffectStorageError) -> Self {
        match value {
            ActiveEffectStorageError::CapacityExceeded { target } => {
                Self::ActiveEffectCapacityExceeded { target }
            }
        }
    }
}

pub(super) fn map_attribute_set_error(
    target: Entity,
    error: AttributeSetError,
) -> GameplayEffectApplicationError {
    match error {
        AttributeSetError::AttributeId(error) => GameplayEffectApplicationError::AttributeId(error),
        AttributeSetError::UninitializedAttribute { id } => {
            GameplayEffectApplicationError::MissingAttribute { target, id }
        }
    }
}

/// Validates an effect application and builds an execution plan without mutation.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] for rejected input or invalid target state.
pub fn prepare_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut EffectSystemParams,
    payload: &EffectPayload,
) -> Result<GameplayEffectApplicationPlan, GameplayEffectApplicationError> {
    let source = payload.get_source();
    let probability = effect_def.get_probability_to_apply();
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
        return Err(GameplayEffectApplicationError::InvalidProbability { probability });
    }
    if probability < 1.0 && !params.random_gen.random_bool(probability) {
        return Err(GameplayEffectApplicationError::ProbabilityRejected);
    }

    let incoming_tags = effect_def.get_tags();
    if !passes_application_requirements(source, target, incoming_tags, params) {
        return Err(GameplayEffectApplicationError::ApplicationRequirementsNotMet);
    }
    if is_blocked_by_application_immunity(source, target, incoming_tags, params)? {
        return Err(GameplayEffectApplicationError::BlockedByImmunity);
    }

    let spec = {
        let context = EffectContext {
            target: Some(target),
            payload,
            attribute_id_manager: &params.attribute_id_manager,
            attr_set_query: &params.attr_set_query.as_readonly(),
            tag_container_query: &params.tag_container_query.as_readonly(),
        };
        effect_def.make_spec(&context)
    };

    if matches!(
        spec.get_duration_spec(),
        EffectDurationTicksSpec::DurationTicks(0)
    ) {
        return Err(GameplayEffectApplicationError::InvalidDuration);
    }
    validate_effect_execution_requirements(
        target,
        &spec,
        &params.attribute_id_manager,
        &params.tag_manager,
        &params.attr_set_query,
        &params.tag_container_query,
        &params.active_effect_query,
    )?;

    let removed_effects = collect_active_effects_with_tags_for_params(
        target,
        incoming_tags.get_remove_effects_with_tags(),
        &params.active_effect_query,
        &params.tag_manager,
    )?;
    if let Some((handle, stack_count)) = find_stackable_active_effect(
        source,
        target,
        &spec,
        &params.active_effect_query,
        &removed_effects,
    ) {
        let stacking_policy = spec.get_stacking_policy();
        let limit = stacking_policy.get_stack_limit();
        if limit != 0 && stack_count >= limit {
            match stacking_policy.get_overflow_policy() {
                StackOverflowPolicy::RejectApplication => {
                    return Err(GameplayEffectApplicationError::StackOverflowRejected);
                }
                StackOverflowPolicy::RefreshDuration => {
                    return Ok(GameplayEffectApplicationPlan {
                        source,
                        target,
                        spec,
                        removed_effects,
                        kind: GameplayEffectApplicationKind::StackExisting {
                            handle,
                            new_stack_count: stack_count,
                        },
                    });
                }
            }
        }
        return Ok(GameplayEffectApplicationPlan {
            source,
            target,
            spec,
            removed_effects,
            kind: GameplayEffectApplicationKind::StackExisting {
                handle,
                new_stack_count: stack_count.saturating_add(1),
            },
        });
    }

    let kind = if spec.get_duration_spec().is_instant() {
        GameplayEffectApplicationKind::Instant
    } else {
        GameplayEffectApplicationKind::CreateActive
    };
    Ok(GameplayEffectApplicationPlan {
        source,
        target,
        spec,
        removed_effects,
        kind,
    })
}
