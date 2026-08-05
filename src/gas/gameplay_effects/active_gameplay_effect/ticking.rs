use super::super::gameplay_effect::StackExpirationPolicy;
use super::super::gameplay_effect_spec::EffectDurationTicksSpec;
use super::execution::{apply_duration_modifiers, apply_instant_modifiers};
use super::planning::GameplayEffectApplicationError;
use super::removal::{EffectCleanupResources, cleanup_effect_state, force_remove_effect};
use super::state::ActiveGameplayEffects;
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{GameplayTagContainer, GameplayTagManager};
use bevy::prelude::*;

/// Advances finite active-effect durations by one fixed tick.
pub fn tick_effect_duration_system(
    mut active_effect_query: Query<(Entity, &mut ActiveGameplayEffects)>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
    mut tag_query: Query<&mut GameplayTagContainer>,
    tag_manager: Res<GameplayTagManager>,
) {
    let mut targets: Vec<Entity> = active_effect_query
        .iter_mut()
        .map(|(target, _)| target)
        .collect();
    targets.sort_by_key(|entity| entity.to_bits());

    for target in targets {
        let Ok((_, mut active_effects)) = active_effect_query.get_mut(target) else {
            continue;
        };
        let handles: Vec<_> = active_effects.handles(target).collect();
        for handle in handles {
            let Some(effect) = active_effects.get_mut(handle) else {
                continue;
            };
            let Some(duration) = effect.duration.as_mut() else {
                continue;
            };
            if duration.remain_ticks > 0 {
                duration.remain_ticks -= 1;
            }
            if duration.remain_ticks != 0 {
                continue;
            }

            if matches!(
                effect
                    .get_spec()
                    .get_stacking_policy()
                    .get_expiration_policy(),
                StackExpirationPolicy::RemoveSingleStack
            ) && effect.get_stack_count() > 1
            {
                let new_stack_count = effect.get_stack_count() - 1;
                effect.set_stack_count(new_stack_count);
                if let EffectDurationTicksSpec::DurationTicks(duration_ticks) =
                    *effect.get_spec().get_duration_spec()
                    && let Some(duration) = effect.duration.as_mut()
                {
                    duration.remain_ticks = duration_ticks;
                }
                let snapshot = effect.clone();
                if !snapshot.is_inhibited()
                    && snapshot.period.is_none()
                    && !snapshot.get_spec().get_modifier_specs().is_empty()
                {
                    let update_result = match attr_query.get_mut(snapshot.get_target()) {
                        Ok(mut attributes) => attributes
                            .remove_modifiers_for_attributes(
                                &attribute_id_manager,
                                handle,
                                snapshot.get_spec().get_modified_attribute_ids(),
                            )
                            .map_err(GameplayEffectApplicationError::from)
                            .and_then(|()| {
                                apply_duration_modifiers(
                                    snapshot.get_target(),
                                    &mut attributes,
                                    &attribute_id_manager,
                                    snapshot.get_spec(),
                                    handle,
                                    snapshot.get_stack_count(),
                                )
                            }),
                        Err(_) => Err(GameplayEffectApplicationError::MissingAttributeSet {
                            target: snapshot.get_target(),
                        }),
                    };
                    if let Err(error) = update_result {
                        error!("failed to update an expiring gameplay effect: {error}");
                        force_remove_effect(
                            handle,
                            &snapshot,
                            &mut active_effects,
                            &mut attr_query,
                            &mut tag_query,
                            &tag_manager,
                        );
                    }
                }
                continue;
            }

            let snapshot = effect.clone();
            match cleanup_effect_state(
                handle,
                &snapshot,
                EffectCleanupResources {
                    attribute_id_manager: &attribute_id_manager,
                    tag_manager: &tag_manager,
                },
                &mut attr_query,
                &mut tag_query,
            ) {
                Ok(()) => {
                    active_effects.remove(handle);
                }
                Err(error) => {
                    error!("failed to clean up an expired gameplay effect: {error}");
                    force_remove_effect(
                        handle,
                        &snapshot,
                        &mut active_effects,
                        &mut attr_query,
                        &mut tag_query,
                        &tag_manager,
                    );
                }
            }
        }
    }
}

/// Advances periodic active effects by one fixed tick.
pub fn tick_effect_period_system(
    mut active_effect_query: Query<(Entity, &mut ActiveGameplayEffects)>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
    mut tag_query: Query<&mut GameplayTagContainer>,
    tag_manager: Res<GameplayTagManager>,
) {
    let mut targets: Vec<Entity> = active_effect_query
        .iter_mut()
        .map(|(target, _)| target)
        .collect();
    targets.sort_by_key(|entity| entity.to_bits());
    for target in targets {
        let Ok((_, mut active_effects)) = active_effect_query.get_mut(target) else {
            continue;
        };
        let handles: Vec<_> = active_effects.handles(target).collect();
        for handle in handles {
            let execution = {
                let Some(effect) = active_effects.get_mut(handle) else {
                    continue;
                };
                if effect.is_inhibited() {
                    continue;
                }
                let Some(period) = effect.period.as_mut() else {
                    continue;
                };
                period.current_tick += 1;
                if period.current_tick < period.period_ticks {
                    continue;
                }
                period.current_tick = 0;
                Some((
                    effect.get_target(),
                    effect.get_spec().clone(),
                    effect.get_stack_count(),
                ))
            };
            let Some((effect_target, spec, stack_count)) = execution else {
                continue;
            };
            if spec.get_modifier_specs().is_empty() {
                continue;
            }
            let result = match attr_query.get_mut(effect_target) {
                Ok(mut attributes) => apply_instant_modifiers(
                    effect_target,
                    &mut attributes,
                    &attribute_id_manager,
                    &spec,
                    stack_count,
                ),
                Err(_) => Err(GameplayEffectApplicationError::MissingAttributeSet {
                    target: effect_target,
                }),
            };
            if let Err(error) = result {
                error!("failed to execute a periodic gameplay effect: {error}");
                if let Some(snapshot) = active_effects.get(handle).cloned() {
                    force_remove_effect(
                        handle,
                        &snapshot,
                        &mut active_effects,
                        &mut attr_query,
                        &mut tag_query,
                        &tag_manager,
                    );
                }
            }
        }
    }
}
