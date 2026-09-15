use super::super::EffectSystemParams;
use super::diagnostics::EffectRequirementMetrics;
use super::lifecycle::{
    EffectCleanupResources, cleanup_effect_state, force_remove_effect,
    install_effect_contributions, remove_effect_contributions,
};
use super::planning::GameplayEffectApplicationError;
use super::state::{ActiveEffectHandle, ActiveGameplayEffect, ActiveGameplayEffects};
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{GameplayTagContainer, GameplayTagManager, TagRequirements};
use bevy::prelude::*;
use std::time::Instant;

#[derive(Resource, Default)]
pub(crate) struct ActiveEffectRequirementSync {
    dirty: bool,
}

impl ActiveEffectRequirementSync {
    pub(super) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn clear(&mut self) {
        self.dirty = false;
    }
}

/// Resolves removal and ongoing tag requirements to a deterministic fixed point.
pub fn resolve_active_effect_tag_requirements(params: &mut EffectSystemParams) {
    params.active_effect_requirement_sync.clear();
    let measuring = params
        .requirement_diagnostics
        .as_ref()
        .is_some_and(|diagnostics| diagnostics.is_enabled());
    let started = measuring.then(Instant::now);
    let mut metrics = EffectRequirementMetrics::default();
    let mut seen_states = Vec::new();
    let mut transitions = Vec::new();
    loop {
        let state = requirement_state_signature(
            &mut params.active_effect_query,
            &params.tag_container_query,
        );
        if measuring {
            metrics.signature_effect_visits += state.len() as u64;
        }
        if let Some((_, transition_start)) = seen_states
            .iter()
            .find(|(seen_state, _)| seen_state == &state)
        {
            let cycle_handles = transitions[*transition_start..].to_vec();
            if measuring {
                metrics.cycles += 1;
            }
            error!(
                "gameplay effect tag requirements entered a non-converging cycle; removing {} participating effects",
                cycle_handles.len()
            );
            remove_nonconverging_active_effects(cycle_handles, params);
            seen_states.clear();
            transitions.clear();
            continue;
        }
        seen_states.push((state, transitions.len()));

        let changed_handles = resolve_tag_requirement_pass(
            &mut params.active_effect_query,
            &mut params.attr_set_query,
            &params.attribute_id_manager,
            &mut params.tag_container_query,
            &params.tag_manager,
            measuring.then_some(&mut metrics),
        );
        if measuring {
            metrics.decision_passes += 1;
            metrics.transitions += changed_handles.len() as u64;
        }
        if changed_handles.is_empty() {
            break;
        }
        transitions.extend(changed_handles);
    }
    if let Some(started) = started
        && let Some(diagnostics) = params.requirement_diagnostics.as_mut()
    {
        metrics.convergence_calls = 1;
        metrics.elapsed = started.elapsed();
        diagnostics.record(metrics);
    }
}

fn remove_nonconverging_active_effects(
    mut handles: Vec<ActiveEffectHandle>,
    params: &mut EffectSystemParams,
) {
    handles.sort_by_key(|handle| {
        (
            handle.get_target().to_bits(),
            handle.get_slot(),
            handle.get_generation(),
        )
    });
    handles.dedup();

    for handle in handles {
        let snapshot = params
            .active_effect_query
            .get(handle.get_target())
            .ok()
            .and_then(|active_effects| active_effects.get(handle))
            .cloned();
        let Some(snapshot) = snapshot else {
            continue;
        };

        let cleanup_result = cleanup_effect_state(
            handle,
            &snapshot,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
            &mut params.attr_set_query,
            &mut params.tag_container_query,
        );
        match cleanup_result {
            Ok(()) => {
                if let Ok(mut active_effects) =
                    params.active_effect_query.get_mut(handle.get_target())
                {
                    active_effects.remove(handle);
                }
            }
            Err(error) => {
                error!("failed to clean up a non-converging gameplay effect: {error}");
                if let Ok(mut active_effects) =
                    params.active_effect_query.get_mut(handle.get_target())
                {
                    force_remove_effect(
                        handle,
                        &snapshot,
                        &mut active_effects,
                        &mut params.attr_set_query,
                        &mut params.tag_container_query,
                        &params.tag_manager,
                    );
                }
            }
        }
    }
}

pub(crate) fn resolve_active_effect_tag_requirements_if_dirty(params: &mut EffectSystemParams) {
    if params.active_effect_requirement_sync.take_dirty() {
        resolve_active_effect_tag_requirements(params);
    }
}

/// Bevy system wrapper for [`resolve_active_effect_tag_requirements`].
pub fn update_active_effect_tag_requirements_system(mut params: EffectSystemParams) {
    resolve_active_effect_tag_requirements(&mut params);
}

fn requirement_state_signature(
    active_effect_query: &mut Query<&mut ActiveGameplayEffects>,
    tag_query: &Query<&mut GameplayTagContainer>,
) -> Vec<(u64, u32, u32, bool, bool, bool)> {
    let mut state = Vec::new();
    for active_effects in active_effect_query.iter_mut() {
        for handle in active_effects.stored_handles() {
            if let Some(effect) = active_effects.get(handle) {
                state.push((
                    handle.get_target().to_bits(),
                    handle.get_slot(),
                    handle.get_generation(),
                    effect.is_inhibited(),
                    should_remove_active_effect(effect, tag_query),
                    passes_ongoing_requirements(effect, tag_query),
                ));
            }
        }
    }
    state.sort_unstable();
    state
}

fn resolve_tag_requirement_pass(
    active_effect_query: &mut Query<&mut ActiveGameplayEffects>,
    attr_query: &mut Query<&mut AttributeSet>,
    attribute_id_manager: &AttributeIdManager,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
    mut metrics: Option<&mut EffectRequirementMetrics>,
) -> Vec<ActiveEffectHandle> {
    let mut targets: Vec<Entity> = active_effect_query
        .iter_mut()
        .flat_map(|active_effects| {
            active_effects
                .stored_handles()
                .map(ActiveEffectHandle::get_target)
                .collect::<Vec<_>>()
        })
        .collect();
    targets.sort_by_key(|entity| entity.to_bits());
    targets.dedup();
    let mut decisions = Vec::new();

    for target in targets {
        let Ok(active_effects) = active_effect_query.get(target) else {
            continue;
        };
        let handles: Vec<_> = active_effects.handles(target).collect();
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.decision_effect_visits += handles.len() as u64;
        }
        for handle in handles {
            let Some(effect) = active_effects.get(handle) else {
                continue;
            };
            let decision = if should_remove_active_effect(effect, tag_query) {
                Some(ActiveEffectRequirementDecision::Remove)
            } else {
                match (
                    passes_ongoing_requirements(effect, tag_query),
                    effect.is_inhibited(),
                ) {
                    (false, false) => Some(ActiveEffectRequirementDecision::Inhibit),
                    (true, true) => Some(ActiveEffectRequirementDecision::Uninhibit),
                    _ => None,
                }
            };
            if let Some(decision) = decision {
                // Only transitions need an owned snapshot for the later mutation pass.
                if let Some(metrics) = metrics.as_deref_mut() {
                    metrics.effect_snapshots += 1;
                }
                decisions.push((handle, effect.clone(), decision));
            }
        }
    }

    let mut changed_handles = Vec::with_capacity(decisions.len());
    for (handle, snapshot, decision) in decisions {
        let Ok(mut active_effects) = active_effect_query.get_mut(handle.get_target()) else {
            continue;
        };
        if active_effects.get(handle).is_none() {
            continue;
        }

        match decision {
            ActiveEffectRequirementDecision::Remove => {
                match cleanup_effect_state(
                    handle,
                    &snapshot,
                    EffectCleanupResources {
                        attribute_id_manager,
                        tag_manager,
                    },
                    attr_query,
                    tag_query,
                ) {
                    Ok(()) => {
                        active_effects.remove(handle);
                    }
                    Err(error) => {
                        error!("failed to clean up a gameplay effect: {error}");
                        force_remove_effect(
                            handle,
                            &snapshot,
                            &mut active_effects,
                            attr_query,
                            tag_query,
                            tag_manager,
                        );
                    }
                }
            }
            ActiveEffectRequirementDecision::Inhibit
            | ActiveEffectRequirementDecision::Uninhibit => {
                let inhibiting = matches!(decision, ActiveEffectRequirementDecision::Inhibit);
                let transition_result = if inhibiting {
                    inhibit_active_effect(
                        handle,
                        &snapshot,
                        attribute_id_manager,
                        attr_query,
                        tag_query,
                        tag_manager,
                    )
                } else {
                    uninhibit_active_effect(
                        handle,
                        &snapshot,
                        attribute_id_manager,
                        attr_query,
                        tag_query,
                        tag_manager,
                    )
                };
                if let Err(error) = transition_result {
                    error!("failed to update gameplay effect requirements: {error}");
                    force_remove_effect(
                        handle,
                        &snapshot,
                        &mut active_effects,
                        attr_query,
                        tag_query,
                        tag_manager,
                    );
                } else if let Some(effect) = active_effects.get_mut(handle) {
                    effect.set_inhibited(inhibiting);
                }
            }
        }
        changed_handles.push(handle);
    }
    changed_handles
}

#[derive(Clone, Copy)]
enum ActiveEffectRequirementDecision {
    Remove,
    Inhibit,
    Uninhibit,
}

fn should_remove_active_effect(
    effect: &ActiveGameplayEffect,
    tag_query: &Query<&mut GameplayTagContainer>,
) -> bool {
    let source_tags = tag_query.get(effect.get_source()).ok();
    let target_tags = tag_query.get(effect.get_target()).ok();
    let effect_tags = effect.get_spec().get_def_tags();
    removal_requirement_matches(effect_tags.get_source_removal_tags(), source_tags)
        || removal_requirement_matches(effect_tags.get_target_removal_tags(), target_tags)
}

fn removal_requirement_matches(
    requirements: &TagRequirements,
    tags: Option<&GameplayTagContainer>,
) -> bool {
    !requirements.is_empty() && requirements.passes(tags)
}

fn passes_ongoing_requirements(
    effect: &ActiveGameplayEffect,
    tag_query: &Query<&mut GameplayTagContainer>,
) -> bool {
    let source_tags = tag_query.get(effect.get_source()).ok();
    let target_tags = tag_query.get(effect.get_target()).ok();
    let effect_tags = effect.get_spec().get_def_tags();
    effect_tags.get_source_ongoing_tags().passes(source_tags)
        && effect_tags.get_target_ongoing_tags().passes(target_tags)
}

fn inhibit_active_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<(), GameplayEffectApplicationError> {
    remove_effect_contributions(
        handle,
        effect.get_spec(),
        EffectCleanupResources {
            attribute_id_manager,
            tag_manager,
        },
        attr_query,
        tag_query,
    )
}

fn uninhibit_active_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<(), GameplayEffectApplicationError> {
    install_effect_contributions(
        handle,
        effect.get_spec(),
        effect.get_stack_count(),
        EffectCleanupResources {
            attribute_id_manager,
            tag_manager,
        },
        attr_query,
        tag_query,
    )
}
