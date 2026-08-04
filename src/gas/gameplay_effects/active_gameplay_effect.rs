use super::gameplay_effect::{
    EffectPayload, GameplayEffect, StackDurationPolicy, StackExpirationPolicy,
    StackMagnitudePolicy, StackOverflowPolicy, StackPeriodPolicy, StackingType,
};
use super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use super::{EffectContext, EffectTags, TagRequirements};
use crate::ability_system::AbilitySystemParams;
use crate::attributes::{
    AttributeId, AttributeIdError, AttributeIdManager, AttributeSet, AttributeSetError,
};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagError, GameplayTagManager,
    tag_bits_from_tags_with_manager,
};
use crate::modifiers::ModifierSpec;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
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
            Self::ApplicationRequirementsNotMet => {
                write!(
                    f,
                    "gameplay effect application tag requirements were not met"
                )
            }
            Self::BlockedByImmunity => {
                write!(f, "gameplay effect application was blocked by immunity")
            }
            Self::InvalidDuration => write!(
                f,
                "gameplay effect duration must be greater than zero ticks"
            ),
            Self::MissingAttributeSet { target } => {
                write!(f, "gameplay effect target {target:?} has no AttributeSet")
            }
            Self::MissingAttribute { target, id } => write!(
                f,
                "gameplay effect target {target:?} has not initialized attribute {}",
                id.to_index()
            ),
            Self::MissingTagContainer { target } => {
                write!(
                    f,
                    "gameplay effect target {target:?} has no GameplayTagContainer"
                )
            }
            Self::StackOverflowRejected => {
                write!(f, "gameplay effect stacking policy rejected overflow")
            }
            Self::GameplayTag(error) => {
                write!(f, "gameplay effect contains an invalid tag: {error}")
            }
            Self::AttributeId(error) => {
                write!(
                    f,
                    "gameplay effect contains an invalid attribute ID: {error}"
                )
            }
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

fn map_attribute_set_error(
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
/// Returns [`GameplayEffectApplicationError`] with the specific rejection,
/// configuration error, or missing target component.
pub fn prepare_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
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
            asc_query: &params.asc_query.as_readonly(),
        };

        effect_def.make_spec(&context)
    };

    let duration_spec = spec.get_duration_spec();
    if matches!(duration_spec, EffectDurationTicksSpec::DurationTicks(0)) {
        return Err(GameplayEffectApplicationError::InvalidDuration);
    }

    validate_effect_execution_requirements(
        target,
        &spec,
        &params.attribute_id_manager,
        &params.tag_manager,
        &params.attr_set_query,
        &params.tag_container_query,
    )?;

    let removed_effects = collect_active_effects_with_tags_for_params(
        target,
        incoming_tags.get_remove_effects_with_tags(),
        &params.active_effect_target_index,
        &mut params.active_effect_query,
        &params.tag_manager,
    )?;

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

    let kind = if duration_spec.is_instant() {
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

/// Executes a previously prepared gameplay effect plan.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if required ECS state changed or
/// a tag or attribute ID is invalid.
pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    validate_gameplay_effect_plan(&plan, params)?;
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
    )?;

    match plan.kind {
        GameplayEffectApplicationKind::Instant => execute_instant_effect(&plan, params),
        GameplayEffectApplicationKind::StackExisting {
            handle,
            new_stack_count,
        } => execute_stack_existing_effect(&plan, handle, new_stack_count, params),
        GameplayEffectApplicationKind::CreateActive => execute_new_active_effect(&plan, params),
    }
}

pub(crate) fn validate_gameplay_effect_plan(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    for &handle in &plan.removed_effects {
        let Ok((_, effect, _, _)) = params.active_effect_query.get_mut(handle) else {
            continue;
        };
        validate_effect_cleanup(
            &effect,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
        )?;
    }

    validate_effect_execution_requirements(
        plan.target,
        &plan.spec,
        &params.attribute_id_manager,
        &params.tag_manager,
        &params.attr_set_query,
        &params.tag_container_query,
    )?;

    if let GameplayEffectApplicationKind::StackExisting { handle, .. } = plan.kind
        && let Ok((_, effect, _, _)) = params.active_effect_query.get_mut(handle)
    {
        validate_effect_execution_requirements(
            effect.get_target(),
            effect.get_spec(),
            &params.attribute_id_manager,
            &params.tag_manager,
            &params.attr_set_query,
            &params.tag_container_query,
        )?;
    }

    Ok(())
}

fn validate_effect_execution_requirements(
    target: Entity,
    spec: &GameplayEffectSpec,
    attribute_id_manager: &AttributeIdManager,
    tag_manager: &Res<GameplayTagManager>,
    attr_query: &Query<&mut AttributeSet>,
    tag_query: &Query<&mut GameplayTagContainer>,
) -> Result<(), GameplayEffectApplicationError> {
    if !spec.get_modifier_specs().is_empty() {
        let Ok(attr_set) = attr_query.get(target) else {
            return Err(GameplayEffectApplicationError::MissingAttributeSet { target });
        };
        for modifier in spec.get_modifier_specs() {
            attr_set
                .validate_initialized_attribute(attribute_id_manager, modifier.get_id())
                .map_err(|error| map_attribute_set_error(target, error))?;
        }
    }
    if !spec.get_def_tags().get_granted_tags().is_empty() && tag_query.get(target).is_err() {
        return Err(GameplayEffectApplicationError::MissingTagContainer { target });
    }
    tag_bits_from_tags_with_manager(spec.get_def_tags().get_granted_tags(), tag_manager)?;
    Ok(())
}

fn validate_effect_cleanup(
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
) -> Result<(), GameplayEffectApplicationError> {
    for id in effect.get_spec().get_modified_attribute_ids() {
        resources.attribute_id_manager.location(id)?;
    }
    tag_bits_from_tags_with_manager(
        effect.get_spec().get_def_tags().get_granted_tags(),
        resources.tag_manager,
    )?;
    Ok(())
}

/// Prepares and executes a gameplay effect application.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] for expected gameplay rejections
/// as well as invalid configuration or runtime state.
pub fn apply_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError> {
    let plan = prepare_gameplay_effect(target, effect_def, params, payload)?;
    execute_gameplay_effect_plan(plan, params)
}

fn execute_instant_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
        return Err(GameplayEffectApplicationError::MissingAttributeSet {
            target: plan.target,
        });
    };
    apply_instant_modifiers(
        plan.target,
        &mut target_attrs_mut,
        &params.attribute_id_manager,
        &plan.spec,
        1,
    )?;
    Ok(())
}

fn execute_stack_existing_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    new_stack_count: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
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
            return Err(GameplayEffectApplicationError::MissingAttributeSet {
                target: existing_target,
            });
        };
        target_attrs_mut.remove_modifiers_for_attributes(
            &params.attribute_id_manager,
            handle,
            existing_spec.get_modified_attribute_ids(),
        )?;
        apply_duration_modifiers(
            existing_target,
            &mut target_attrs_mut,
            &params.attribute_id_manager,
            &existing_spec,
            handle,
            new_stack_count,
        )?;
    }

    Ok(())
}

fn execute_new_active_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let has_modifiers = !plan.spec.get_modifier_specs().is_empty();
    let grants_tags = !plan.spec.get_def_tags().get_granted_tags().is_empty();
    if grants_tags && params.tag_container_query.get(plan.target).is_err() {
        return Err(GameplayEffectApplicationError::MissingTagContainer {
            target: plan.target,
        });
    }

    let mut entity_cmds = params.commands.spawn(ActiveGameplayEffect::new(
        plan.spec.clone(),
        plan.source,
        plan.target,
    ));

    let effect_entity = entity_cmds.id();

    if let EffectDurationTicksSpec::DurationTicks(duration) = plan.spec.get_duration_spec() {
        entity_cmds.insert(ActiveEffectDurationTicks {
            remain_ticks: *duration,
        });
    }

    // Apply granted tags before modifiers so that tag failures don't leave
    // partially-applied modifier state behind.
    if grants_tags {
        let Ok(mut target_tags) = params.tag_container_query.get_mut(plan.target) else {
            params.commands.entity(effect_entity).despawn();
            return Err(GameplayEffectApplicationError::MissingTagContainer {
                target: plan.target,
            });
        };
        if let Err(error) = target_tags.add_tags(
            plan.spec.get_def_tags().get_granted_tags(),
            &params.tag_manager,
        ) {
            params.commands.entity(effect_entity).despawn();
            return Err(error.into());
        }
    }

    if let Some(period_spec) = plan.spec.get_period_spec() {
        let period_ticks = period_spec.get_period_ticks();
        let execute_on_application = period_spec.get_execute_on_applied();
        if period_ticks == 0 {
            // period_ticks == 0 is a no-op; fall through to duration modifiers
            // so the effect still applies its modifiers at least once.
            if has_modifiers {
                let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
                    rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
                    return Err(GameplayEffectApplicationError::MissingAttributeSet {
                        target: plan.target,
                    });
                };
                if let Err(error) = apply_duration_modifiers(
                    plan.target,
                    &mut target_attrs_mut,
                    &params.attribute_id_manager,
                    &plan.spec,
                    effect_entity,
                    1,
                ) {
                    rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
                    return Err(error);
                }
            }
        } else {
            if execute_on_application && has_modifiers {
                let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
                    rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
                    return Err(GameplayEffectApplicationError::MissingAttributeSet {
                        target: plan.target,
                    });
                };
                if let Err(error) = apply_instant_modifiers(
                    plan.target,
                    &mut target_attrs_mut,
                    &params.attribute_id_manager,
                    &plan.spec,
                    1,
                ) {
                    rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
                    return Err(error);
                }
            }
            entity_cmds.insert(ActiveEffectPeriodTicks {
                period_ticks,
                current_tick: 0,
            });
        }
    } else if has_modifiers {
        let Ok(mut target_attrs_mut) = params.attr_set_query.get_mut(plan.target) else {
            rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
            return Err(GameplayEffectApplicationError::MissingAttributeSet {
                target: plan.target,
            });
        };
        if let Err(error) = apply_duration_modifiers(
            plan.target,
            &mut target_attrs_mut,
            &params.attribute_id_manager,
            &plan.spec,
            effect_entity,
            1,
        ) {
            rollback_pending_effect(plan, effect_entity, grants_tags, params)?;
            return Err(error);
        }
    }

    entity_cmds.set_parent_in_place(plan.target);
    params
        .active_effect_target_index
        .add(plan.target, effect_entity);

    Ok(())
}

fn rollback_pending_effect(
    plan: &GameplayEffectApplicationPlan,
    effect_entity: Entity,
    tags_applied: bool,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(mut attr_set) = params.attr_set_query.get_mut(plan.target) {
        attr_set.remove_modifiers(effect_entity);
    }
    let tag_result = if tags_applied {
        match params.tag_container_query.get_mut(plan.target) {
            Ok(mut tags) => tags
                .remove_tags(
                    plan.spec.get_def_tags().get_granted_tags(),
                    &params.tag_manager,
                )
                .map_err(GameplayEffectApplicationError::from),
            Err(_) => Err(GameplayEffectApplicationError::MissingTagContainer {
                target: plan.target,
            }),
        }
    } else {
        Ok(())
    };
    params.commands.entity(effect_entity).despawn();
    tag_result?;
    Ok(())
}

/// Removes an active effect and cleans up its modifiers and granted tags.
///
/// Returns `Ok(false)` when `handle` does not identify an active effect.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if cleanup encounters an invalid
/// tag or attribute ID.
pub fn remove_active_effect(
    handle: ActiveEffectHandle,
    params: &mut AbilitySystemParams,
) -> Result<bool, GameplayEffectApplicationError> {
    let Ok((_, effect, _, _)) = params.active_effect_query.get_mut(handle) else {
        return Ok(false);
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
    )?;
    Ok(true)
}

/// Removes active effects on `target` whose asset tags match `tags`.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if matching or cleanup encounters
/// an invalid tag or attribute ID.
pub fn remove_active_effects_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams,
) -> Result<usize, GameplayEffectApplicationError> {
    let handles = collect_active_effects_with_tags_for_params(
        target,
        tags,
        &params.active_effect_target_index,
        &mut params.active_effect_query,
        &params.tag_manager,
    )?;
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
    )?;
    Ok(removed_count)
}

pub fn get_active_effects_on_target(
    target: Entity,
    target_index: &ActiveGameplayEffectTargetIndex,
) -> Vec<ActiveEffectHandle> {
    target_index.handles_for(target).to_vec()
}

/// Tests whether `target` has an active effect matching any supplied tag.
///
/// # Errors
///
/// Returns [`GameplayTagError::InvalidTagIndex`] if any queried or effect tag is
/// not registered in `tag_manager`.
pub fn has_active_effect_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    target_index: &ActiveGameplayEffectTargetIndex,
    active_effect_query: &Query<(Entity, &ActiveGameplayEffect)>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<bool, GameplayTagError> {
    if tags.is_empty() {
        return Ok(false);
    }

    for handle in target_index.handles_for(target).iter().copied() {
        let Ok((_, effect)) = active_effect_query.get(handle) else {
            continue;
        };
        if effect.get_target() == target && active_effect_has_any_tags(effect, tags, tag_manager)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn cleanup_active_gameplay_effect(
    commands: &mut Commands,
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    target_index: &mut ActiveGameplayEffectTargetIndex,
) -> Result<(), GameplayEffectApplicationError> {
    validate_effect_cleanup(effect, resources)?;
    if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
        attr_set.remove_modifiers_for_attributes(
            resources.attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        )?;
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            resources.tag_manager,
        )?;
    }

    commands.entity(handle).despawn();
    target_index.remove(effect.get_target(), handle);
    Ok(())
}

fn remove_failed_active_effect(
    commands: &mut Commands,
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    target_index: &mut ActiveGameplayEffectTargetIndex,
) {
    if let Err(error) = cleanup_active_gameplay_effect(
        commands,
        handle,
        effect,
        resources,
        attr_query,
        tag_query,
        target_index,
    ) {
        error!("failed to clean up an invalid gameplay effect: {error}");
        if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
            attr_set.remove_modifiers(handle);
        }
        discard_active_gameplay_effect(commands, handle, effect, target_index);
    }
}

fn discard_active_gameplay_effect(
    commands: &mut Commands,
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    target_index: &mut ActiveGameplayEffectTargetIndex,
) {
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

                let update_result =
                    if !effect.is_inhibited() && effect.get_spec().get_period_spec().is_none() {
                        match attr_query.get_mut(effect.get_target()) {
                            Ok(mut attr_set) => attr_set
                                .remove_modifiers_for_attributes(
                                    &attribute_id_manager,
                                    entity,
                                    effect.get_spec().get_modified_attribute_ids(),
                                )
                                .map_err(GameplayEffectApplicationError::from)
                                .and_then(|()| {
                                    apply_duration_modifiers(
                                        effect.get_target(),
                                        &mut attr_set,
                                        &attribute_id_manager,
                                        effect.get_spec(),
                                        entity,
                                        effect.get_stack_count(),
                                    )
                                }),
                            Err(_) => Err(GameplayEffectApplicationError::MissingAttributeSet {
                                target: effect.get_target(),
                            }),
                        }
                    } else {
                        Ok(())
                    };
                if let Err(error) = update_result {
                    error!("failed to update an expiring gameplay effect: {error}");
                    remove_failed_active_effect(
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
                continue;
            }

            if let Err(error) = cleanup_active_gameplay_effect(
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
            ) {
                error!("failed to clean up an expired gameplay effect: {error}");
                discard_active_gameplay_effect(&mut commands, entity, &effect, &mut target_index);
            }
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
            if let Err(error) = cleanup_active_gameplay_effect(
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
            ) {
                error!("failed to clean up a gameplay effect: {error}");
                discard_active_gameplay_effect(&mut commands, handle, &effect, &mut target_index);
            }
            continue;
        }

        let ongoing_passes = passes_ongoing_requirements(&effect, &tag_query);
        match (ongoing_passes, effect.is_inhibited()) {
            (false, false) => {
                if let Err(error) = inhibit_active_effect(
                    handle,
                    &mut effect,
                    &attribute_id_manager,
                    &mut attr_query,
                    &mut tag_query,
                    &tag_manager,
                ) {
                    error!("failed to inhibit a gameplay effect: {error}");
                    remove_failed_active_effect(
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
                }
            }
            (true, true) => {
                if let Err(error) = uninhibit_active_effect(
                    handle,
                    &mut effect,
                    &attribute_id_manager,
                    &mut attr_query,
                    &mut tag_query,
                    &tag_manager,
                ) {
                    error!("failed to uninhibit a gameplay effect: {error}");
                    remove_failed_active_effect(
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
                }
            }
            _ => {}
        }
    }
}

pub fn tick_effect_period_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ActiveEffectPeriodTicks, &ActiveGameplayEffect)>,
    mut attr_query: Query<&mut AttributeSet>,
    attribute_id_manager: Res<AttributeIdManager>,
    mut tag_query: Query<&mut GameplayTagContainer>,
    tag_manager: Res<GameplayTagManager>,
    mut target_index: ResMut<ActiveGameplayEffectTargetIndex>,
) {
    for (handle, mut period, effect) in query.iter_mut() {
        if effect.is_inhibited() {
            continue;
        }

        period.current_tick += 1;
        if period.current_tick >= period.period_ticks {
            period.current_tick = 0;
            let execution_result = match attr_query.get_mut(effect.get_target()) {
                Ok(mut attr_set) => apply_instant_modifiers(
                    effect.get_target(),
                    &mut attr_set,
                    &attribute_id_manager,
                    effect.get_spec(),
                    effect.get_stack_count(),
                ),
                Err(_) => Err(GameplayEffectApplicationError::MissingAttributeSet {
                    target: effect.get_target(),
                }),
            };
            if let Err(error) = execution_result {
                error!("failed to execute a periodic gameplay effect: {error}");
                remove_failed_active_effect(
                    &mut commands,
                    handle,
                    effect,
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
) -> Result<bool, GameplayTagError> {
    let source_tags = params.tag_container_query.get(source).ok();
    let incoming_asset_bits =
        tag_bits_from_tags_with_manager(incoming_tags.get_asset_tags(), &params.tag_manager)?;

    Ok(params
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
                .any(|immunity| immunity.matches_tag_bits(source_tags, Some(&incoming_asset_bits)))
        }))
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
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(mut attr_set) = attr_query.get_mut(effect.get_target()) {
        attr_set.remove_modifiers_for_attributes(
            attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        )?;
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )?;
    }

    effect.set_inhibited(true);
    Ok(())
}

fn uninhibit_active_effect(
    handle: ActiveEffectHandle,
    effect: &mut ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<(), GameplayEffectApplicationError> {
    if effect.get_spec().get_period_spec().is_none()
        && let Ok(mut attr_set) = attr_query.get_mut(effect.get_target())
    {
        apply_duration_modifiers(
            effect.get_target(),
            &mut attr_set,
            attribute_id_manager,
            effect.get_spec(),
            handle,
            effect.get_stack_count(),
        )?;
    }

    if let Ok(mut tag_container) = tag_query.get_mut(effect.get_target()) {
        tag_container.add_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )?;
    }

    effect.set_inhibited(false);
    Ok(())
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
) -> Result<Vec<ActiveEffectHandle>, GameplayTagError> {
    if tags.is_empty() {
        return Ok(Vec::new());
    }

    let mut matches = Vec::new();
    for handle in target_index.handles_for(target).iter().copied() {
        let Ok((_, effect, _, _)) = active_effect_query.get_mut(handle) else {
            continue;
        };
        if effect.get_target() == target && active_effect_has_any_tags(&effect, tags, tag_manager)?
        {
            matches.push(handle);
        }
    }
    Ok(matches)
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
) -> Result<(), GameplayEffectApplicationError> {
    for &handle in handles {
        let Ok((_, effect, _, _)) = active_effect_query.get_mut(handle) else {
            continue;
        };
        validate_effect_cleanup(&effect, resources)?;
    }
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
        )?;
    }
    Ok(())
}

fn apply_duration_modifiers(
    target: Entity,
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    handle: ActiveEffectHandle,
    stack_count: u32,
) -> Result<(), GameplayEffectApplicationError> {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for mod_spec in spec.get_modifier_specs() {
        let stacked_spec = mod_spec.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_duration_modifier(attribute_id_manager, &stacked_spec, handle)
            .map_err(|error| map_attribute_set_error(target, error))?;
    }
    Ok(())
}

fn apply_instant_modifiers(
    target: Entity,
    attr_set: &mut AttributeSet,
    attribute_id_manager: &AttributeIdManager,
    spec: &GameplayEffectSpec,
    stack_count: u32,
) -> Result<(), GameplayEffectApplicationError> {
    let stack_multiplier = stack_multiplier(
        spec.get_stacking_policy().get_magnitude_policy(),
        stack_count,
    );
    for mod_spec in spec.get_modifier_specs() {
        let stacked_spec = mod_spec.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_instant_modifier(attribute_id_manager, &stacked_spec)
            .map_err(|error| map_attribute_set_error(target, error))?;
    }
    Ok(())
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
) -> Result<bool, GameplayTagError> {
    let effect_bits = tag_bits_from_tags_with_manager(
        effect.get_spec().get_def_tags().get_asset_tags(),
        tag_manager,
    )?;
    let query_bits = tag_bits_from_tags_with_manager(tags, tag_manager)?;

    Ok(effect_bits
        .iter()
        .zip(query_bits.iter())
        .any(|(a, b)| (a & b) != 0))
}
