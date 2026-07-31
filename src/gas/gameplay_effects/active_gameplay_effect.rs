use super::gameplay_effect::{
    EffectPayload, GameplayEffect, StackDurationPolicy, StackExpirationPolicy,
    StackMagnitudePolicy, StackOverflowPolicy, StackPeriodPolicy, StackingType,
};
use super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use super::{EffectContext, EffectTags, TagRequirements};
use crate::ability_system::AbilitySystemParams;
use crate::attributes::{AttributeIdManager, AttributeSet};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagManager, tag_bits_from_tags_with_manager,
};
use crate::modifiers::ModifierSpec;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct EffectCleanupResources<'a, 'w> {
    attribute_id_manager: &'a AttributeIdManager,
    tag_manager: &'a Res<'w, GameplayTagManager>,
}

pub type ActiveEffectHandle = Entity;

#[derive(Resource, Default)]
pub struct ActiveGameplayEffectTargetIndex {
    by_target: HashMap<Entity, Vec<ActiveEffectHandle>>,
    by_handle: HashMap<ActiveEffectHandle, Entity>,
}

impl ActiveGameplayEffectTargetIndex {
    pub fn add(&mut self, target: Entity, handle: ActiveEffectHandle) {
        self.by_target.entry(target).or_default().push(handle);
        self.by_handle.insert(handle, target);
    }

    pub fn remove(&mut self, target: Entity, handle: ActiveEffectHandle) {
        let Some(handles) = self.by_target.get_mut(&target) else {
            self.by_handle.remove(&handle);
            return;
        };
        handles.retain(|&candidate| candidate != handle);
        if handles.is_empty() {
            self.by_target.remove(&target);
        }
        self.by_handle.remove(&handle);
    }

    pub fn remove_by_handle(&mut self, handle: ActiveEffectHandle) {
        if let Some(target) = self.by_handle.get(&handle).copied() {
            self.remove(target, handle);
        }
    }

    pub fn handles_for(&self, target: Entity) -> &[ActiveEffectHandle] {
        self.by_target
            .get(&target)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

pub fn reconcile_active_effect_target_index_system(
    mut removed_effects: RemovedComponents<ActiveGameplayEffect>,
    mut target_index: ResMut<ActiveGameplayEffectTargetIndex>,
) {
    for handle in removed_effects.read() {
        target_index.remove_by_handle(handle);
    }
}

#[derive(Component, Clone)]
pub struct ActiveGameplayEffect {
    spec: GameplayEffectSpec,
    source: Entity,
    target: Entity,
    stack_count: u32,
    inhibited: bool,
}

impl ActiveGameplayEffect {
    pub fn new(spec: GameplayEffectSpec, source: Entity, target: Entity) -> Self {
        Self {
            spec,
            source,
            target,
            stack_count: 1,
            inhibited: false,
        }
    }

    pub fn get_spec(&self) -> &GameplayEffectSpec {
        &self.spec
    }

    pub fn get_source(&self) -> Entity {
        self.source
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_stack_count(&self) -> u32 {
        self.stack_count
    }

    pub fn set_stack_count(&mut self, stack_count: u32) {
        self.stack_count = stack_count.max(1);
    }

    pub fn is_inhibited(&self) -> bool {
        self.inhibited
    }

    pub fn set_inhibited(&mut self, inhibited: bool) {
        self.inhibited = inhibited;
    }
}

impl GameplayEffectApplicationPlan {
    pub fn get_modifier_specs(&self) -> &[ModifierSpec] {
        self.spec.get_modifier_specs()
    }

    pub fn is_instant(&self) -> bool {
        matches!(self.kind, GameplayEffectApplicationKind::Instant)
    }
}

#[derive(Component)]
pub struct ActiveEffectDurationTicks {
    remain_ticks: u32,
}

#[derive(Component)]
pub struct ActiveEffectPeriodTicks {
    period_ticks: u32,
    current_tick: u32,
}

pub struct GameplayEffectApplicationPlan {
    source: Entity,
    target: Entity,
    spec: GameplayEffectSpec,
    removed_effects: Vec<ActiveEffectHandle>,
    kind: GameplayEffectApplicationKind,
}

enum GameplayEffectApplicationKind {
    Instant,
    StackExisting {
        handle: ActiveEffectHandle,
        new_stack_count: u32,
    },
    CreateActive,
}

pub fn prepare_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> Option<GameplayEffectApplicationPlan> {
    let source = payload.get_source();

    let probability = effect_def.get_probability_to_apply();
    if probability < 1.0 && !params.random_gen.random_bool(probability) {
        return None;
    }

    let incoming_tags = effect_def.get_tags();
    if !passes_application_requirements(source, target, incoming_tags, params) {
        return None;
    }

    if is_blocked_by_application_immunity(source, target, incoming_tags, params) {
        return None;
    }

    let spec = {
        let context = EffectContext {
            target: Some(target),
            payload,
            attribute_id_manager: &params.attribute_id_manager,
            attr_set_query: &params.attr_set_query.as_readonly(),
            tag_container_query: &params.tag_container_query.as_readonly(),
            asc_query: &params.asc_query.as_readonly(),
        };

        effect_def.make_spec(&context)
    };

    let duration_spec = spec.get_duration_spec();
    if matches!(duration_spec, EffectDurationTicksSpec::DurationTicks(0)) {
        return None;
    }

    let has_modifiers = !spec.get_modifier_specs().is_empty();
    let needs_attribute_set = has_modifiers
        && (duration_spec.is_instant()
            || spec.get_period_spec().is_none()
            || spec.get_period_spec().as_ref().is_some_and(|period| {
                period.get_execute_on_applied() || period.get_period_ticks() > 0
            }));
    if needs_attribute_set && params.attr_set_query.get(target).is_err() {
        return None;
    }

    let grants_tags = !spec.get_def_tags().get_granted_tags().is_empty();
    if grants_tags && params.tag_container_query.get(target).is_err() {
        return None;
    }

    let removed_effects = collect_active_effects_with_tags_for_params(
        target,
        incoming_tags.get_remove_effects_with_tags(),
        &params.active_effect_target_index,
        &mut params.active_effect_query,
        &params.tag_manager,
    );

    if let Some((handle, stack_count)) = find_stackable_active_effect(
        source,
        target,
        &spec,
        &mut params.active_effect_query,
        &params.active_effect_target_index,
        &removed_effects,
    ) {
        let stacking_policy = spec.get_stacking_policy();
        let limit = stacking_policy.get_stack_limit();
        if limit != 0 && stack_count >= limit {
            match stacking_policy.get_overflow_policy() {
                StackOverflowPolicy::RejectApplication => return None,
                StackOverflowPolicy::RefreshDuration => {
                    return Some(GameplayEffectApplicationPlan {
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

        return Some(GameplayEffectApplicationPlan {
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

    let kind = if duration_spec.is_instant() {
        GameplayEffectApplicationKind::Instant
    } else {
        GameplayEffectApplicationKind::CreateActive
    };

    Some(GameplayEffectApplicationPlan {
        source,
        target,
        spec,
        removed_effects,
        kind,
    })
}

pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> bool {
    remove_collected_active_effects_for_params(
        &plan.removed_effects,
        &mut params.active_effect_query,
        &mut params.commands,
        EffectCleanupResources {
            attribute_id_manager: &params.attribute_id_manager,
            tag_manager: &params.tag_manager,
        },
        &mut params.attr_set_query,
        &mut params.tag_container_query,
        &mut params.active_effect_target_index,
    );

    match plan.kind {
        GameplayEffectApplicationKind::Instant => execute_instant_effect(&plan, params),
        GameplayEffectApplicationKind::StackExisting {
            handle,
            new_stack_count,
        } => execute_stack_existing_effect(&plan, handle, new_stack_count, params),
        GameplayEffectApplicationKind::CreateActive => execute_new_active_effect(&plan, params),
    }
}

pub fn apply_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> bool {
    let Some(plan) = prepare_gameplay_effect(target, effect_def, params, payload) else {
        return false;
    };

    execute_gameplay_effect_plan(plan, params)
}

fn execute_instant_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> bool {
    let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
        return false;
    };
    apply_instant_modifiers(
        &mut target_attrs_mut,
        &params.attribute_id_manager,
        &plan.spec,
        1,
    );
    true
}

fn execute_stack_existing_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    new_stack_count: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    let Ok((_, mut active_effect, duration, period)) = params.active_effect_query.get_mut(handle)
    else {
        return execute_new_active_effect(plan, params);
    };

    active_effect.set_stack_count(new_stack_count);
    let existing_target = active_effect.get_target();
    let existing_spec = active_effect.get_spec().clone();

    if matches!(
        plan.spec.get_stacking_policy().get_duration_policy(),
        StackDurationPolicy::RefreshOnSuccessfulStack
    ) && let (EffectDurationTicksSpec::DurationTicks(duration_ticks), Some(mut duration)) =
        (plan.spec.get_duration_spec(), duration)
    {
        duration.remain_ticks = *duration_ticks;
    }

    if matches!(
        plan.spec.get_stacking_policy().get_period_policy(),
        StackPeriodPolicy::ResetOnSuccessfulStack
    ) && let Some(mut period) = period
    {
        period.current_tick = 0;
    }

    if !active_effect.is_inhibited() && existing_spec.get_period_spec().is_none() {
        let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(existing_target) else {
            return false;
        };
        target_attrs_mut.remove_modifiers_for_attributes(
            &params.attribute_id_manager,
            handle,
            existing_spec.get_modified_attribute_ids(),
        );
        apply_duration_modifiers(
            &mut target_attrs_mut,
            &params.attribute_id_manager,
            &existing_spec,
            handle,
            new_stack_count,
        );
    }

    true
}

fn execute_new_active_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> bool {
    let has_modifiers = !plan.spec.get_modifier_specs().is_empty();
    let grants_tags = !plan.spec.get_def_tags().get_granted_tags().is_empty();
    if grants_tags && params.tag_container_query.get(plan.target).is_err() {
        return false;
    }

    let mut entity_cmds = params.commands.spawn(ActiveGameplayEffect::new(
        plan.spec.clone(),
        plan.source,
        plan.target,
    ));

    let effect_entity = entity_cmds.id();
    params
        .active_effect_target_index
        .add(plan.target, effect_entity);

    if let EffectDurationTicksSpec::DurationTicks(duration) = plan.spec.get_duration_spec() {
        entity_cmds.insert(ActiveEffectDurationTicks {
            remain_ticks: *duration,
        });
    }

    // Apply granted tags before modifiers so that tag failures don't leave
    // partially-applied modifier state behind.
    if grants_tags {
        let Ok(mut target_tags) = params.tag_container_query.get_mut(plan.target) else {
            params
                .active_effect_target_index
                .remove(plan.target, effect_entity);
            params.commands.entity(effect_entity).despawn();
            return false;
        };
        target_tags.add_tags(
            plan.spec.get_def_tags().get_granted_tags(),
            &params.tag_manager,
        );
    }

    if let Some(period_spec) = plan.spec.get_period_spec() {
        let period_ticks = period_spec.get_period_ticks();
        let execute_on_application = period_spec.get_execute_on_applied();
        if period_ticks == 0 {
            // period_ticks == 0 is a no-op; fall through to duration modifiers
            // so the effect still applies its modifiers at least once.
            if has_modifiers {
                let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
                    // Roll back tags that were already applied above.
                    if grants_tags
                        && let Ok(mut target_tags) = params.tag_container_query.get_mut(plan.target)
                    {
                        target_tags.remove_tags(
                            plan.spec.get_def_tags().get_granted_tags(),
                            &params.tag_manager,
                        );
                    }
                    params
                        .active_effect_target_index
                        .remove(plan.target, effect_entity);
                    params.commands.entity(effect_entity).despawn();
                    return false;
                };
                apply_duration_modifiers(
                    &mut target_attrs_mut,
                    &params.attribute_id_manager,
                    &plan.spec,
                    effect_entity,
                    1,
                );
            }
        } else {
            if execute_on_application && has_modifiers {
                let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
                    // Roll back tags that were already applied above.
                    if grants_tags
                        && let Ok(mut target_tags) = params.tag_container_query.get_mut(plan.target)
                    {
                        target_tags.remove_tags(
                            plan.spec.get_def_tags().get_granted_tags(),
                            &params.tag_manager,
                        );
                    }
                    params
                        .active_effect_target_index
                        .remove(plan.target, effect_entity);
                    params.commands.entity(effect_entity).despawn();
                    return false;
                };
                apply_instant_modifiers(
                    &mut target_attrs_mut,
                    &params.attribute_id_manager,
                    &plan.spec,
                    1,
                );
            }
            entity_cmds.insert(ActiveEffectPeriodTicks {
                period_ticks,
                current_tick: 0,
            });
        }
    } else if has_modifiers {
        let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
            // Roll back tags that were already applied above.
            if grants_tags
                && let Ok(mut target_tags) = params.tag_container_query.get_mut(plan.target)
            {
                target_tags.remove_tags(
                    plan.spec.get_def_tags().get_granted_tags(),
                    &params.tag_manager,
                );
            }
            params
                .active_effect_target_index
                .remove(plan.target, effect_entity);
            params.commands.entity(effect_entity).despawn();
            return false;
        };
        apply_duration_modifiers(
            &mut target_attrs_mut,
            &params.attribute_id_manager,
            &plan.spec,
            effect_entity,
            1,
        );
    }

    entity_cmds.set_parent_in_place(plan.target);

    true
}

pub fn remove_active_effect(handle: ActiveEffectHandle, params: &mut AbilitySystemParams) -> bool {
    let Ok((_, effect, _, _)) = params.active_effect_query.get_mut(handle) else {
        return false;
    };
    let effect = effect.clone();
    cleanup_active_gameplay_effect(
        &mut params.commands,
        handle,
        &effect,
        EffectCleanupResources {
            attribute_id_manager: &params.attribute_id_manager,
            tag_manager: &params.tag_manager,
        },
        &mut params.attr_set_query,
        &mut params.tag_container_query,
        &mut params.active_effect_target_index,
    );
    true
}

pub fn remove_active_effects_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams,
) -> usize {
    let handles = collect_active_effects_with_tags_for_params(
        target,
        tags,
        &params.active_effect_target_index,
        &mut params.active_effect_query,
        &params.tag_manager,
    );
    let removed_count = handles.len();
    remove_collected_active_effects_for_params(
        &handles,
        &mut params.active_effect_query,
        &mut params.commands,
        EffectCleanupResources {
            attribute_id_manager: &params.attribute_id_manager,
            tag_manager: &params.tag_manager,
        },
        &mut params.attr_set_query,
        &mut params.tag_container_query,
        &mut params.active_effect_target_index,
    );
    removed_count
}

pub fn get_active_effects_on_target(
    target: Entity,
    target_index: &ActiveGameplayEffectTargetIndex,
) -> Vec<ActiveEffectHandle> {
    target_index.handles_for(target).to_vec()
}

pub fn has_active_effect_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    target_index: &ActiveGameplayEffectTargetIndex,
    active_effect_query: &Query<(Entity, &ActiveGameplayEffect)>,
    tag_manager: &Res<GameplayTagManager>,
) -> bool {
    if tags.is_empty() {
        return false;
    }

    target_index
        .handles_for(target)
        .iter()
        .copied()
        .any(|handle| {
            let Ok((_, effect)) = active_effect_query.get(handle) else {
                return false;
            };
            effect.get_target() == target && active_effect_has_any_tags(effect, tags, tag_manager)
        })
}

fn cleanup_active_gameplay_effect(
    commands: &mut Commands,
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    target_index: &mut ActiveGameplayEffectTargetIndex,
) {
    if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
        attr_set.remove_modifiers_for_attributes(
            resources.attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        );
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            resources.tag_manager,
        );
    }

    commands.entity(handle).despawn();
    target_index.remove(effect.get_target(), handle);
}

pub fn tick_effect_duration_system(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut ActiveEffectDurationTicks,
        &mut ActiveGameplayEffect,
    )>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
    mut tag_query: Query<&mut GameplayTagContainer>,
    tag_manager: Res<GameplayTagManager>,
    mut target_index: ResMut<ActiveGameplayEffectTargetIndex>,
) {
    for (entity, mut duration, mut effect) in query.iter_mut() {
        if duration.remain_ticks > 0 {
            duration.remain_ticks -= 1;
        }

        if duration.remain_ticks == 0 {
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
                    effect.get_spec().get_duration_spec()
                {
                    duration.remain_ticks = *duration_ticks;
                }

                if !effect.is_inhibited()
                    && effect.get_spec().get_period_spec().is_none()
                    && let Ok(mut attr_set) = attr_query.get_mut(effect.get_target())
                {
                    attr_set.remove_modifiers_for_attributes(
                        &attribute_id_manager,
                        entity,
                        effect.get_spec().get_modified_attribute_ids(),
                    );
                    apply_duration_modifiers(
                        &mut attr_set,
                        &attribute_id_manager,
                        effect.get_spec(),
                        entity,
                        effect.get_stack_count(),
                    );
                }
                continue;
            }

            cleanup_active_gameplay_effect(
                &mut commands,
                entity,
                &effect,
                EffectCleanupResources {
                    attribute_id_manager: &attribute_id_manager,
                    tag_manager: &tag_manager,
                },
                &mut attr_query,
                &mut tag_query,
                &mut target_index,
            );
        }
    }
}

pub fn update_active_effect_tag_requirements_system(
    mut commands: Commands,
    mut active_effect_query: Query<(Entity, &mut ActiveGameplayEffect)>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
    mut tag_query: Query<&mut GameplayTagContainer>,
    tag_manager: Res<GameplayTagManager>,
    mut target_index: ResMut<ActiveGameplayEffectTargetIndex>,
) {
    for (handle, mut effect) in active_effect_query.iter_mut() {
        if should_remove_active_effect(&effect, &tag_query) {
            cleanup_active_gameplay_effect(
                &mut commands,
                handle,
                &effect,
                EffectCleanupResources {
                    attribute_id_manager: &attribute_id_manager,
                    tag_manager: &tag_manager,
                },
                &mut attr_query,
                &mut tag_query,
                &mut target_index,
            );
            continue;
        }

        let ongoing_passes = passes_ongoing_requirements(&effect, &tag_query);
        match (ongoing_passes, effect.is_inhibited()) {
            (false, false) => {
                inhibit_active_effect(
                    handle,
                    &mut effect,
                    &attribute_id_manager,
                    &mut attr_query,
                    &mut tag_query,
                    &tag_manager,
                );
            }
            (true, true) => {
                uninhibit_active_effect(
                    handle,
                    &mut effect,
                    &attribute_id_manager,
                    &mut attr_query,
                    &mut tag_query,
                    &tag_manager,
                );
            }
            _ => {}
        }
    }
}

pub fn tick_effect_period_system(
    mut query: Query<(&mut ActiveEffectPeriodTicks, &ActiveGameplayEffect)>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
) {
    for (mut period, effect) in query.iter_mut() {
        if effect.is_inhibited() {
            continue;
        }

        period.current_tick += 1;
        if period.current_tick >= period.period_ticks {
            period.current_tick = 0;
            if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
                apply_instant_modifiers(
                    &mut attr_set,
                    &attribute_id_manager,
                    effect.get_spec(),
                    effect.get_stack_count(),
                );
            }
        }
    }
}

fn find_stackable_active_effect(
    source: Entity,
    target: Entity,
    spec: &GameplayEffectSpec,
    active_effect_query: &mut Query<(
        Entity,
        &mut ActiveGameplayEffect,
        Option<&mut ActiveEffectDurationTicks>,
        Option<&mut ActiveEffectPeriodTicks>,
    )>,
    target_index: &ActiveGameplayEffectTargetIndex,
    ignored_handles: &[ActiveEffectHandle],
) -> Option<(ActiveEffectHandle, u32)> {
    let stacking_type = spec.get_stacking_policy().get_stacking_type();
    if matches!(stacking_type, StackingType::None) {
        return None;
    }

    target_index
        .handles_for(target)
        .iter()
        .copied()
        .find_map(|handle| {
            if ignored_handles.contains(&handle) {
                return None;
            }
            let Ok((_, effect, _, _)) = active_effect_query.get_mut(handle) else {
                return None;
            };
            (effect.get_target() == target
                && spec.is_same_def(effect.get_spec())
                && match stacking_type {
                    StackingType::None => false,
                    StackingType::AggregateBySource => effect.get_source() == source,
                    StackingType::AggregateByTarget => true,
                })
            .then_some((handle, effect.get_stack_count()))
        })
}

fn passes_application_requirements(
    source: Entity,
    target: Entity,
    incoming_tags: &EffectTags,
    params: &AbilitySystemParams,
) -> bool {
    let source_tags = params.tag_container_query.get(source).ok();
    let target_tags = params.tag_container_query.get(target).ok();

    incoming_tags
        .get_source_application_tags()
        .passes(source_tags)
        && incoming_tags
            .get_target_application_tags()
            .passes(target_tags)
}

fn is_blocked_by_application_immunity(
    source: Entity,
    target: Entity,
    incoming_tags: &EffectTags,
    params: &mut AbilitySystemParams,
) -> bool {
    let source_tags = params.tag_container_query.get(source).ok();
    let incoming_asset_bits =
        tag_bits_from_tags_with_manager(incoming_tags.get_asset_tags(), &params.tag_manager);

    params
        .active_effect_target_index
        .handles_for(target)
        .to_vec()
        .into_iter()
        .any(|handle| {
            let Ok((_, active_effect, _, _)) = params.active_effect_query.get_mut(handle) else {
                return false;
            };
            if active_effect.is_inhibited() {
                return false;
            }
            active_effect
                .get_spec()
                .get_def_tags()
                .get_granted_application_immunity()
                .iter()
                .any(|immunity| {
                    immunity.matches_tag_bits(source_tags, incoming_asset_bits.as_ref())
                })
        })
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
    effect: &mut ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) {
    if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
        attr_set.remove_modifiers_for_attributes(
            attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        );
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        );
    }

    effect.set_inhibited(true);
}

fn uninhibit_active_effect(
    handle: ActiveEffectHandle,
    effect: &mut ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) {
    if effect.get_spec().get_period_spec().is_none()
        && let Ok(mut attr_set) = attr_query.get_mut(effect.get_target())
    {
        apply_duration_modifiers(
            &mut attr_set,
            attribute_id_manager,
            effect.get_spec(),
            handle,
            effect.get_stack_count(),
        );
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.add_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        );
    }

    effect.set_inhibited(false);
}

fn collect_active_effects_with_tags_for_params(
    target: Entity,
    tags: &[GameplayTag],
    target_index: &ActiveGameplayEffectTargetIndex,
    active_effect_query: &mut Query<(
        Entity,
        &mut ActiveGameplayEffect,
        Option<&mut ActiveEffectDurationTicks>,
        Option<&mut ActiveEffectPeriodTicks>,
    )>,
    tag_manager: &Res<GameplayTagManager>,
) -> Vec<ActiveEffectHandle> {
    if tags.is_empty() {
        return Vec::new();
    }

    target_index
        .handles_for(target)
        .iter()
        .copied()
        .filter(|&handle| {
            let Ok((_, effect, _, _)) = active_effect_query.get_mut(handle) else {
                return false;
            };
            effect.get_target() == target && active_effect_has_any_tags(&effect, tags, tag_manager)
        })
        .collect()
}

fn remove_collected_active_effects_for_params(
    handles: &[ActiveEffectHandle],
    active_effect_query: &mut Query<(
        Entity,
        &mut ActiveGameplayEffect,
        Option<&mut ActiveEffectDurationTicks>,
        Option<&mut ActiveEffectPeriodTicks>,
    )>,
    commands: &mut Commands,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    target_index: &mut ActiveGameplayEffectTargetIndex,
) {
    for &handle in handles {
        let Ok((_, effect, _, _)) = active_effect_query.get_mut(handle) else {
            continue;
        };
        let effect = effect.clone();
        cleanup_active_gameplay_effect(
            commands,
            handle,
            &effect,
            resources,
            attr_query,
            tag_query,
            target_index,
        );
    }
}

fn apply_duration_modifiers(
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    handle: ActiveEffectHandle,
    stack_count: u32,
) {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for mod_spec in spec.get_modifier_specs() {
        let stacked_spec = mod_spec.scaled_by_stack(stack_multiplier);
        attr_set.apply_duration_modifier(attribute_id_manager, &stacked_spec, handle);
    }
}

fn apply_instant_modifiers(
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    stack_count: u32,
) {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for mod_spec in spec.get_modifier_specs() {
        let stacked_spec = mod_spec.scaled_by_stack(stack_multiplier);
        attr_set.apply_instant_modifier(attribute_id_manager, &stacked_spec);
    }
}

fn stack_multiplier(policy: StackMagnitudePolicy, stack_count: u32) -> u32 {
    match policy {
        StackMagnitudePolicy::None => 1,
        StackMagnitudePolicy::Linear => stack_count,
    }
}

fn active_effect_has_any_tags(
    effect: &ActiveGameplayEffect,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> bool {
    let Some(effect_bits) = tag_bits_from_tags_with_manager(
        effect.get_spec().get_def_tags().get_asset_tags(),
        tag_manager,
    ) else {
        return false;
    };
    let Some(query_bits) = tag_bits_from_tags_with_manager(tags, tag_manager) else {
        return false;
    };

    effect_bits
        .iter()
        .zip(query_bits.iter())
        .any(|(a, b)| (a & b) != 0)
}
