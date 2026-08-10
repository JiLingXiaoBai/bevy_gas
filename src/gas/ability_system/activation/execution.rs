use super::super::commit::{execute_ability_commit_plans, prepare_ability_commit_plans};
use super::super::lifecycle::{cancel_active_abilities_with_tags, finish_ability_with_status};
use super::super::params::AbilitySystemParams;
use super::error::{AbilityActivationError, ability_activation_failed};
use super::startup::{StartupAbilityTaskContext, start_startup_ability_tasks};
use super::validation::passes_ability_activation_requirements;
use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityActivationStatus, AbilitySpecHandle,
    effect_payload_from_ability_context,
};
use crate::gameplay_effects::{
    apply_gameplay_effect_in_batch, resolve_active_effect_tag_requirements,
    resolve_active_effect_tag_requirements_if_dirty,
};
use crate::gameplay_execution::{
    AbilityActivationRequest, GameplayExecutionQueue, drain_gameplay_execution_queue,
};
use crate::gameplay_tags::tag_bits_from_tags_with_manager;
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::*;

/// Activates an ability through an independent synchronous call path and drains all startup Instant
/// follow-up requests before returning.
///
/// This function does not consume or order itself against the global [`GameplayExecutionQueue`].
/// Runtime producer systems should enqueue activations instead of mixing this immediate API with
/// already queued mutations in the same logical phase. "Synchronous" describes logical resolution,
/// not transactional rollback or an immediate flush of entity changes queued through [`Commands`].
///
/// # Errors
///
/// Returns [`AbilityActivationError`] when validation, commit, cancellation, or startup fails.
pub fn try_activate_ability_by_handle(
    source: Entity,
    targets: impl Into<AbilityActivationTargets>,
    handle: AbilitySpecHandle,
    activation_context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError> {
    resolve_active_effect_tag_requirements(&mut params.effects);
    params
        .pending_active_abilities
        .retain_unapplied(&params.active_ability_query);
    let mut execution_queue = GameplayExecutionQueue::default();
    let result = execute_ability_activation_in_batch(
        AbilityActivationRequest::new(source, targets, handle, activation_context),
        &mut execution_queue,
        params,
    );
    drain_gameplay_execution_queue(&mut execution_queue, params);
    result
}

pub(crate) fn execute_ability_activation_in_batch(
    request: AbilityActivationRequest,
    execution_queue: &mut GameplayExecutionQueue,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError> {
    let source = request.get_source();
    let handle = request.get_handle();
    let activation_context = request.get_context();

    if let Some(chain) = activation_context.get_chain()
        && let Err(error) = chain.validate_for_handle(handle)
    {
        return ability_activation_failed(AbilityActivationError::InvalidChain(error));
    }

    let (ability, level, active_count) = {
        let Ok(asc) = params.asc_query.get(source) else {
            return ability_activation_failed(
                AbilityActivationError::MissingAbilitySystemComponent { source },
            );
        };
        let Some(spec) = asc.find_ability_spec(handle) else {
            return ability_activation_failed(AbilityActivationError::AbilityNotFound {
                source,
                handle,
            });
        };
        (
            spec.get_ability().clone(),
            spec.get_level(),
            spec.get_active_count(),
        )
    };

    if !ability.allow_multiple_instances() && active_count > 0 {
        return ability_activation_failed(AbilityActivationError::MultipleInstancesNotAllowed {
            source,
            handle,
        });
    }

    if !passes_ability_activation_requirements(source, &ability, params) {
        return ability_activation_failed(AbilityActivationError::ActivationRequirementsNotMet {
            source,
            handle,
        });
    }

    let commit_plans = match prepare_ability_commit_plans(
        source,
        &ability,
        level,
        Some(activation_context),
        params,
    ) {
        Ok(plans) => plans,
        Err(error) => {
            return ability_activation_failed(AbilityActivationError::CommitPreparationFailed {
                source,
                handle,
                error,
            });
        }
    };

    if let Err(error) = tag_bits_from_tags_with_manager(
        ability.get_tags().get_block_abilities_with_tags(),
        &params.effects.tag_manager,
    ) {
        return ability_activation_failed(AbilityActivationError::StartFailed {
            source,
            handle,
            error,
        });
    }

    if let Err(error) = cancel_active_abilities_with_tags(
        source,
        ability.get_tags().get_cancel_abilities_with_tags(),
        params,
    ) {
        return ability_activation_failed(AbilityActivationError::CancellationFailed {
            source,
            handle,
            error,
        });
    }

    let active_handle = {
        let Ok(mut asc) = params.asc_query.get_mut(source) else {
            return ability_activation_failed(
                AbilityActivationError::MissingAbilitySystemComponent { source },
            );
        };
        match asc.start_ability(
            &request,
            &mut params.commands,
            &params.effects.tag_manager,
            &mut params.pending_active_abilities,
        ) {
            Ok(active_handle) => active_handle,
            Err(error) => {
                return ability_activation_failed(AbilityActivationError::StartFailed {
                    source,
                    handle,
                    error,
                });
            }
        }
    };

    if let Err(error) = execute_ability_commit_plans(commit_plans, params) {
        if let Ok(mut asc) = params.asc_query.get_mut(source)
            && let Err(rollback_error) = asc.rollback_started_ability(
                active_handle,
                handle,
                &mut params.commands,
                &params.effects.tag_manager,
            )
        {
            asc.discard_started_ability(active_handle, handle, &mut params.commands);
            params.pending_active_abilities.remove(active_handle);
            return ability_activation_failed(AbilityActivationError::StartFailed {
                source,
                handle,
                error: rollback_error,
            });
        }
        params.pending_active_abilities.remove(active_handle);
        return ability_activation_failed(AbilityActivationError::CommitExecutionFailed {
            source,
            handle,
            error,
        });
    }
    resolve_active_effect_tag_requirements_if_dirty(&mut params.effects);

    for effect in ability.get_activation_effects() {
        for activation_target in request.get_targets().entities() {
            let payload =
                effect_payload_from_ability_context(source, level, Some(activation_context));
            if let Err(error) = apply_gameplay_effect_in_batch(
                activation_target,
                effect,
                &mut params.effects,
                &payload,
            ) {
                if error.is_rejection() {
                    debug!("ability activation effect was rejected: {error}");
                } else {
                    error!("ability activation effect failed: {error}");
                }
            }
            resolve_active_effect_tag_requirements_if_dirty(&mut params.effects);
        }
    }

    let startup_ends_ability = start_startup_ability_tasks(
        ability.get_startup_tasks(),
        StartupAbilityTaskContext {
            active_handle,
            request: &request,
            level,
        },
        execution_queue,
        params,
    );

    if startup_ends_ability || ability.should_end_on_activation() {
        finish_ability_with_status(
            source,
            active_handle,
            AbilityActivationStatus::Ending,
            params,
        );
    }

    Ok(())
}
