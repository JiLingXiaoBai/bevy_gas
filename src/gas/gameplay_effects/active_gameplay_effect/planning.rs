use super::super::gameplay_effect::{EffectPayload, GameplayEffect, StackOverflowPolicy};
use super::super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use super::super::{EffectContext, EffectSystemParams};
use super::application::{
    find_stackable_active_effect, is_blocked_by_application_immunity,
    passes_application_requirements,
};
use super::error::{GameplayEffectApplicationError, map_attribute_set_error};
use super::execution::validate_effect_execution_requirements;
use super::removal::collect_active_effects_with_tags_for_params;
use super::state::{ActiveEffectHandle, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet, AttributeSnapshot};
use crate::gameplay_tags::GameplayTagManager;
use crate::modifiers::{ModifierSourceId, ModifierSpec};
use bevy::prelude::*;
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

    /// Previews this plan's modifier sequence after removing its selected effect sources.
    ///
    /// Calls `accepts` after each modifier and returns `false` on the first rejected
    /// snapshot. Attribute state and post-execute callbacks remain untouched.
    ///
    /// Returns an application error if the target or a modified attribute is missing.
    pub(crate) fn preview_modifier_application(
        &self,
        params: &EffectSystemParams,
        accepts: impl FnMut(AttributeSnapshot) -> bool,
    ) -> Result<bool, GameplayEffectApplicationError> {
        preview_modifiers_after_effect_removal(
            self.target,
            &self.spec,
            &self.removed_effects,
            &params.attr_set_query.as_readonly(),
            &params.attribute_id_manager,
            accepts,
        )
    }

    pub(super) fn changes_active_effect_requirements(&self) -> bool {
        !self.removed_effects.is_empty()
            || matches!(self.kind, GameplayEffectApplicationKind::CreateActive)
    }
}

/// Previews instant modifiers with the effect sources selected by removal tags excluded.
///
/// This only inspects the target and evaluates attribute values. It does not check
/// application requirements, roll probability, invoke post-execute callbacks, or mutate ECS state.
/// Calls `accepts` after each modifier and returns `false` on the first rejected snapshot.
///
/// Returns an application error for invalid removal tags or missing target attributes.
pub(crate) fn preview_instant_effect_modifiers(
    target: Entity,
    spec: &GameplayEffectSpec,
    attributes: &Query<&AttributeSet>,
    active_effects: &Query<&ActiveGameplayEffects>,
    attribute_ids: &AttributeIdManager,
    tag_manager: &Res<GameplayTagManager>,
    accepts: impl FnMut(AttributeSnapshot) -> bool,
) -> Result<bool, GameplayEffectApplicationError> {
    let removed_effects = collect_active_effects_with_tags_for_params(
        target,
        spec.get_def_tags().get_remove_effects_with_tags(),
        active_effects,
        tag_manager,
    )?;
    preview_modifiers_after_effect_removal(
        target,
        spec,
        &removed_effects,
        attributes,
        attribute_ids,
        accepts,
    )
}

fn preview_modifiers_after_effect_removal(
    target: Entity,
    spec: &GameplayEffectSpec,
    removed_effects: &[ActiveEffectHandle],
    attributes: &Query<&AttributeSet>,
    attribute_ids: &AttributeIdManager,
    accepts: impl FnMut(AttributeSnapshot) -> bool,
) -> Result<bool, GameplayEffectApplicationError> {
    let attributes = attributes
        .get(target)
        .map_err(|_| GameplayEffectApplicationError::MissingAttributeSet { target })?;
    attributes
        .preview_instant_modifiers(
            attribute_ids,
            spec.get_modifier_specs(),
            removed_effects.iter().copied().map(ModifierSourceId::from),
            accepts,
        )
        .map_err(|error| map_attribute_set_error(target, error))
}

/// Validates an effect application and builds a plan without modifying gameplay components.
///
/// Probabilistic applications advance the shared random stream before tag, immunity, and
/// structural checks. A rejected application can therefore consume a random draw.
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
        &params.active_effect_query.as_readonly(),
    )?;

    let removed_effects = collect_active_effects_with_tags_for_params(
        target,
        incoming_tags.get_remove_effects_with_tags(),
        &params.active_effect_query.as_readonly(),
        &params.tag_manager,
    )?;
    if let Some((handle, stack_count)) = find_stackable_active_effect(
        source,
        target,
        &spec,
        &params.active_effect_query.as_readonly(),
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
