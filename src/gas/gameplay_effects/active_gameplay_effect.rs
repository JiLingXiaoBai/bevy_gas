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
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct EffectCleanupResources<'a, 'w> {
    attribute_id_manager: &'a AttributeIdManager,
    tag_manager: &'a Res<'w, GameplayTagManager>,
}

#[derive(Resource, Default)]
pub(crate) struct ActiveEffectRequirementSync {
    dirty: bool,
}

impl ActiveEffectRequirementSync {
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn clear(&mut self) {
        self.dirty = false;
    }
}

/// Stable generational handle for an active effect stored on its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActiveEffectHandle {
    target: Entity,
    slot: u32,
    generation: u32,
}

impl ActiveEffectHandle {
    /// Creates a handle from its target, slot, and generation.
    pub const fn new(target: Entity, slot: u32, generation: u32) -> Self {
        Self {
            target,
            slot,
            generation,
        }
    }

    /// Returns the entity that owns the active-effect container.
    pub const fn get_target(self) -> Entity {
        self.target
    }

    /// Returns the stable slot index within the target container.
    pub const fn get_slot(self) -> u32 {
        self.slot
    }

    /// Returns the slot generation used to reject stale handles.
    pub const fn get_generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone)]
struct ActiveEffectSlot {
    generation: u32,
    effect: Option<ActiveGameplayEffect>,
}

/// Target-owned, stable-order storage for active gameplay effects.
#[derive(Component, Default)]
pub struct ActiveGameplayEffects {
    slots: Vec<ActiveEffectSlot>,
    free_slots: Vec<u32>,
}

impl ActiveGameplayEffects {
    /// Returns the number of active effects.
    pub fn len(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.effect.is_some())
            .count()
    }

    /// Returns whether the container has no active effects.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.effect.is_none())
    }

    /// Returns the active effect identified by `handle`.
    pub fn get(&self, handle: ActiveEffectHandle) -> Option<&ActiveGameplayEffect> {
        let slot = self.slots.get(handle.slot as usize)?;
        let effect = slot.effect.as_ref()?;
        (slot.generation == handle.generation && effect.get_target() == handle.target)
            .then_some(effect)
    }

    /// Returns the mutable active effect identified by `handle`.
    pub(crate) fn get_mut(
        &mut self,
        handle: ActiveEffectHandle,
    ) -> Option<&mut ActiveGameplayEffect> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        let effect = slot.effect.as_mut()?;
        (slot.generation == handle.generation && effect.get_target() == handle.target)
            .then_some(effect)
    }

    /// Iterates active handles in stable slot order for `target`.
    pub fn handles(&self, target: Entity) -> impl Iterator<Item = ActiveEffectHandle> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(move |(slot_index, slot)| {
                slot.effect
                    .as_ref()
                    .filter(|effect| effect.get_target() == target)
                    .map(|_| ActiveEffectHandle::new(target, slot_index as u32, slot.generation))
            })
    }

    fn stored_handles(&self) -> impl Iterator<Item = ActiveEffectHandle> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot_index, slot)| {
                slot.effect.as_ref().map(|effect| {
                    ActiveEffectHandle::new(effect.get_target(), slot_index as u32, slot.generation)
                })
            })
    }

    fn insert(
        &mut self,
        target: Entity,
        effect: ActiveGameplayEffect,
    ) -> Result<ActiveEffectHandle, GameplayEffectApplicationError> {
        if let Some(slot_index) = self.free_slots.pop() {
            let slot = &mut self.slots[slot_index as usize];
            debug_assert!(slot.effect.is_none());
            slot.effect = Some(effect);
            return Ok(ActiveEffectHandle::new(target, slot_index, slot.generation));
        }

        let slot_index = u32::try_from(self.slots.len())
            .map_err(|_| GameplayEffectApplicationError::ActiveEffectCapacityExceeded { target })?;
        let generation = 1;
        self.slots.push(ActiveEffectSlot {
            generation,
            effect: Some(effect),
        });
        Ok(ActiveEffectHandle::new(target, slot_index, generation))
    }

    fn remove(&mut self, handle: ActiveEffectHandle) -> Option<ActiveGameplayEffect> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        if slot.effect.as_ref()?.get_target() != handle.target {
            return None;
        }
        let effect = slot.effect.take()?;
        if slot.generation < u32::MAX {
            slot.generation += 1;
            self.free_slots.push(handle.slot);
        }
        Some(effect)
    }
}

/// Runtime state for a gameplay effect stored in [`ActiveGameplayEffects`].
#[derive(Clone)]
pub struct ActiveGameplayEffect {
    spec: GameplayEffectSpec,
    source: Entity,
    target: Entity,
    stack_count: u32,
    inhibited: bool,
    duration: Option<ActiveEffectDurationTicks>,
    period: Option<ActiveEffectPeriodTicks>,
}

impl ActiveGameplayEffect {
    fn new(spec: GameplayEffectSpec, source: Entity, target: Entity) -> Self {
        let duration = match spec.get_duration_spec() {
            EffectDurationTicksSpec::DurationTicks(remain_ticks) => {
                Some(ActiveEffectDurationTicks {
                    remain_ticks: *remain_ticks,
                })
            }
            EffectDurationTicksSpec::Instant | EffectDurationTicksSpec::Infinite => None,
        };
        let period = spec
            .get_period_spec()
            .filter(|period| period.get_period_ticks() > 0)
            .map(|period| ActiveEffectPeriodTicks {
                period_ticks: period.get_period_ticks(),
                current_tick: 0,
            });
        Self {
            spec,
            source,
            target,
            stack_count: 1,
            inhibited: false,
            duration,
            period,
        }
    }

    /// Returns the captured effect specification.
    pub fn get_spec(&self) -> &GameplayEffectSpec {
        &self.spec
    }

    /// Returns the entity that applied the effect.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the entity that owns the effect.
    pub fn get_target(&self) -> Entity {
        self.target
    }

    /// Returns the current stack count.
    pub fn get_stack_count(&self) -> u32 {
        self.stack_count
    }

    /// Sets a positive stack count.
    pub(crate) fn set_stack_count(&mut self, stack_count: u32) {
        self.stack_count = stack_count.max(1);
    }

    /// Returns whether ongoing tag requirements currently inhibit the effect.
    pub fn is_inhibited(&self) -> bool {
        self.inhibited
    }

    fn set_inhibited(&mut self, inhibited: bool) {
        self.inhibited = inhibited;
    }

    /// Returns the optional remaining-duration state.
    pub fn get_duration(&self) -> Option<&ActiveEffectDurationTicks> {
        self.duration.as_ref()
    }

    /// Returns the optional period state.
    pub fn get_period(&self) -> Option<&ActiveEffectPeriodTicks> {
        self.period.as_ref()
    }
}

/// Remaining fixed ticks for a finite active effect.
#[derive(Debug, Clone, Copy)]
pub struct ActiveEffectDurationTicks {
    remain_ticks: u32,
}

impl ActiveEffectDurationTicks {
    /// Returns the remaining fixed ticks.
    pub const fn get_remaining_ticks(&self) -> u32 {
        self.remain_ticks
    }
}

/// Fixed-tick state for a periodic active effect.
#[derive(Debug, Clone, Copy)]
pub struct ActiveEffectPeriodTicks {
    period_ticks: u32,
    current_tick: u32,
}

impl ActiveEffectPeriodTicks {
    /// Returns the configured period in fixed ticks.
    pub const fn get_period_ticks(&self) -> u32 {
        self.period_ticks
    }

    /// Returns the elapsed ticks in the current period.
    pub const fn get_current_tick(&self) -> u32 {
        self.current_tick
    }
}

/// Prepared gameplay-effect work intended for immediate execution.
///
/// Execution revalidates structural ECS requirements, but it does not repeat
/// application-tag, immunity, probability, or stacking decisions captured here.
/// A plan is not a transactional or long-lived command.
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

impl GameplayEffectApplicationPlan {
    /// Returns the prepared modifier specifications.
    pub fn get_modifier_specs(&self) -> &[ModifierSpec] {
        self.spec.get_modifier_specs()
    }

    /// Returns whether this plan applies an instant effect.
    pub fn is_instant(&self) -> bool {
        matches!(self.kind, GameplayEffectApplicationKind::Instant)
    }

    fn changes_active_effect_requirements(&self) -> bool {
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
/// Returns [`GameplayEffectApplicationError`] for rejected input or invalid target state.
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

/// Executes an immediately prepared gameplay-effect plan.
///
/// This revalidates structural ECS requirements, but does not repeat application
/// requirements, immunity, probability, or stacking decisions. It also does not
/// provide transactional rollback if a later mutation fails.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if required ECS state changed.
pub fn execute_gameplay_effect_plan(
    plan: GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements_if_dirty(params);
    let result = execute_gameplay_effect_plan_in_batch(plan, params);
    resolve_active_effect_tag_requirements_if_dirty(params);
    result
}

pub(crate) fn execute_gameplay_effect_plan_in_batch(
    plan: GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    validate_gameplay_effect_plan(&plan, params)?;
    if plan.changes_active_effect_requirements() {
        params.active_effect_requirement_sync.mark_dirty();
    }
    remove_collected_active_effects_for_params(&plan.removed_effects, params)?;
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
    if let Ok(active_effects) = params.active_effect_query.get(plan.target) {
        for &handle in &plan.removed_effects {
            if let Some(effect) = active_effects.get(handle) {
                validate_effect_cleanup(
                    effect,
                    EffectCleanupResources {
                        attribute_id_manager: &params.attribute_id_manager,
                        tag_manager: &params.tag_manager,
                    },
                )?;
            }
        }
    }
    validate_effect_execution_requirements(
        plan.target,
        &plan.spec,
        &params.attribute_id_manager,
        &params.tag_manager,
        &params.attr_set_query,
        &params.tag_container_query,
        &params.active_effect_query,
    )?;
    Ok(())
}

fn validate_effect_execution_requirements(
    target: Entity,
    spec: &GameplayEffectSpec,
    attribute_id_manager: &AttributeIdManager,
    tag_manager: &Res<GameplayTagManager>,
    attr_query: &Query<&mut AttributeSet>,
    tag_query: &Query<&mut GameplayTagContainer>,
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
) -> Result<(), GameplayEffectApplicationError> {
    if !spec.get_duration_spec().is_instant() && active_effect_query.get(target).is_err() {
        return Err(GameplayEffectApplicationError::MissingActiveGameplayEffects { target });
    }
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

/// Prepares and executes a gameplay-effect application through an independent synchronous call path.
///
/// Active-effect tag requirements are converged before preparation and again before this function
/// returns. Runtime producer systems should enqueue applications when ordering against the global
/// [`GameplayExecutionQueue`](crate::GameplayExecutionQueue) matters. This call does not provide
/// transactional rollback if execution fails after earlier gameplay mutations.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] for rejection or invalid runtime state.
pub fn apply_gameplay_effect(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let result = apply_gameplay_effect_in_batch(target, effect_def, params, payload);
    resolve_active_effect_tag_requirements_if_dirty(params);
    result
}

pub(crate) fn apply_gameplay_effect_in_batch(
    target: Entity,
    effect_def: &Arc<GameplayEffect>,
    params: &mut AbilitySystemParams,
    payload: &EffectPayload,
) -> Result<(), GameplayEffectApplicationError> {
    let plan = prepare_gameplay_effect(target, effect_def, params, payload)?;
    execute_gameplay_effect_plan_in_batch(plan, params)
}

fn execute_instant_effect(
    plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if plan.spec.get_modifier_specs().is_empty() {
        return Ok(());
    }

    let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
        return Err(GameplayEffectApplicationError::MissingAttributeSet {
            target: plan.target,
        });
    };
    apply_instant_modifiers(
        plan.target,
        &mut attributes,
        &params.attribute_id_manager,
        &plan.spec,
        1,
    )
}

fn execute_stack_existing_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    new_stack_count: u32,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let (target, spec, inhibited, runtime_periodic) = {
        let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) else {
            return execute_new_active_effect(plan, params);
        };
        let Some(effect) = active_effects.get_mut(handle) else {
            return execute_new_active_effect(plan, params);
        };
        effect.set_stack_count(new_stack_count);
        if matches!(
            plan.spec.get_stacking_policy().get_duration_policy(),
            StackDurationPolicy::RefreshOnSuccessfulStack
        ) && let (EffectDurationTicksSpec::DurationTicks(duration_ticks), Some(duration)) =
            (plan.spec.get_duration_spec(), effect.duration.as_mut())
        {
            duration.remain_ticks = *duration_ticks;
        }
        if matches!(
            plan.spec.get_stacking_policy().get_period_policy(),
            StackPeriodPolicy::ResetOnSuccessfulStack
        ) && let Some(period) = effect.period.as_mut()
        {
            period.current_tick = 0;
        }
        (
            effect.get_target(),
            effect.get_spec().clone(),
            effect.is_inhibited(),
            effect.period.is_some(),
        )
    };

    if !inhibited && !runtime_periodic && !spec.get_modifier_specs().is_empty() {
        let Ok(mut attributes) = params.attr_set_query.get_mut(target) else {
            return Err(GameplayEffectApplicationError::MissingAttributeSet { target });
        };
        attributes.remove_modifiers_for_attributes(
            &params.attribute_id_manager,
            handle,
            spec.get_modified_attribute_ids(),
        )?;
        apply_duration_modifiers(
            target,
            &mut attributes,
            &params.attribute_id_manager,
            &spec,
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
    let handle = {
        let Ok(mut active_effects) = params.active_effect_query.get_mut(plan.target) else {
            return Err(
                GameplayEffectApplicationError::MissingActiveGameplayEffects {
                    target: plan.target,
                },
            );
        };
        active_effects.insert(
            plan.target,
            ActiveGameplayEffect::new(plan.spec.clone(), plan.source, plan.target),
        )?
    };

    if grants_tags {
        let Ok(mut tags) = params.tag_container_query.get_mut(plan.target) else {
            rollback_new_active_effect(plan, handle, false, params)?;
            return Err(GameplayEffectApplicationError::MissingTagContainer {
                target: plan.target,
            });
        };
        if let Err(error) = tags.add_tags(
            plan.spec.get_def_tags().get_granted_tags(),
            &params.tag_manager,
        ) {
            rollback_new_active_effect(plan, handle, false, params)?;
            return Err(error.into());
        }
    }

    if let Some(period_spec) = plan.spec.get_period_spec() {
        if period_spec.get_period_ticks() == 0 {
            if has_modifiers {
                let result = apply_new_duration_modifiers(plan, handle, params);
                if let Err(error) = result {
                    rollback_new_active_effect(plan, handle, grants_tags, params)?;
                    return Err(error);
                }
            }
        } else if period_spec.get_execute_on_applied() && has_modifiers {
            let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
                rollback_new_active_effect(plan, handle, grants_tags, params)?;
                return Err(GameplayEffectApplicationError::MissingAttributeSet {
                    target: plan.target,
                });
            };
            if let Err(error) = apply_instant_modifiers(
                plan.target,
                &mut attributes,
                &params.attribute_id_manager,
                &plan.spec,
                1,
            ) {
                rollback_new_active_effect(plan, handle, grants_tags, params)?;
                return Err(error);
            }
        }
    } else if has_modifiers {
        let result = apply_new_duration_modifiers(plan, handle, params);
        if let Err(error) = result {
            rollback_new_active_effect(plan, handle, grants_tags, params)?;
            return Err(error);
        }
    }
    Ok(())
}

fn apply_new_duration_modifiers(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) else {
        return Err(GameplayEffectApplicationError::MissingAttributeSet {
            target: plan.target,
        });
    };
    apply_duration_modifiers(
        plan.target,
        &mut attributes,
        &params.attribute_id_manager,
        &plan.spec,
        handle,
        1,
    )
}

fn rollback_new_active_effect(
    plan: &GameplayEffectApplicationPlan,
    handle: ActiveEffectHandle,
    tags_applied: bool,
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    if let Ok(mut attributes) = params.attr_set_query.get_mut(plan.target) {
        attributes.remove_modifiers(handle);
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
    if let Ok(mut active_effects) = params.active_effect_query.get_mut(plan.target) {
        active_effects.remove(handle);
    }
    tag_result
}

/// Removes an active effect and cleans up its modifiers and granted tags.
///
/// Returns `Ok(false)` for a stale or missing handle.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if cleanup fails.
pub fn remove_active_effect(
    handle: ActiveEffectHandle,
    params: &mut AbilitySystemParams,
) -> Result<bool, GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let effect = {
        let Ok(active_effects) = params.active_effect_query.get(handle.get_target()) else {
            return Ok(false);
        };
        let Some(effect) = active_effects.get(handle) else {
            return Ok(false);
        };
        effect.clone()
    };
    cleanup_effect_state(
        handle,
        &effect,
        EffectCleanupResources {
            attribute_id_manager: &params.attribute_id_manager,
            tag_manager: &params.tag_manager,
        },
        &mut params.attr_set_query,
        &mut params.tag_container_query,
    )?;
    if let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) {
        active_effects.remove(handle);
    }
    params.active_effect_requirement_sync.mark_dirty();
    resolve_active_effect_tag_requirements_if_dirty(params);
    Ok(true)
}

/// Removes active effects on `target` whose asset tags match `tags`.
///
/// # Errors
///
/// Returns [`GameplayEffectApplicationError`] if matching or cleanup fails.
pub fn remove_active_effects_with_tags(
    target: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams,
) -> Result<usize, GameplayEffectApplicationError> {
    resolve_active_effect_tag_requirements(params);
    let handles = collect_active_effects_with_tags_for_params(
        target,
        tags,
        &params.active_effect_query,
        &params.tag_manager,
    )?;
    let removed_count = handles.len();
    remove_collected_active_effects_for_params(&handles, params)?;
    if removed_count > 0 {
        params.active_effect_requirement_sync.mark_dirty();
        resolve_active_effect_tag_requirements_if_dirty(params);
    }
    Ok(removed_count)
}

/// Returns active handles in stable slot order.
pub fn get_active_effects_on_target(
    target: Entity,
    active_effects: &ActiveGameplayEffects,
) -> Vec<ActiveEffectHandle> {
    active_effects.handles(target).collect()
}

/// Tests whether a container has an active effect matching any supplied tag.
///
/// # Errors
///
/// Returns [`GameplayTagError`] when a queried tag is invalid.
pub fn has_active_effect_with_tags(
    active_effects: &ActiveGameplayEffects,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> Result<bool, GameplayTagError> {
    if tags.is_empty() {
        return Ok(false);
    }
    for slot in &active_effects.slots {
        let Some(effect) = slot.effect.as_ref() else {
            continue;
        };
        if active_effect_has_any_tags(effect, tags, tag_manager)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn cleanup_effect_state(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    resources: EffectCleanupResources,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
) -> Result<(), GameplayEffectApplicationError> {
    validate_effect_cleanup(effect, resources)?;
    if effect.is_inhibited() {
        return Ok(());
    }
    if let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) {
        attributes.remove_modifiers_for_attributes(
            resources.attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        )?;
    }
    if let Ok(mut tags) = tag_query.get_mut(effect.get_target()) {
        tags.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            resources.tag_manager,
        )?;
    }
    Ok(())
}

fn force_remove_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    active_effects: &mut ActiveGameplayEffects,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) {
    if let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) {
        attributes.remove_modifiers(handle);
    }
    if !effect.is_inhibited()
        && let Ok(mut tags) = tag_query.get_mut(effect.get_target())
        && let Err(error) = tags.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )
    {
        error!("failed to force-remove gameplay effect tags: {error}");
    }
    active_effects.remove(handle);
}

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

/// Resolves removal and ongoing tag requirements to a deterministic fixed point.
pub fn resolve_active_effect_tag_requirements(params: &mut AbilitySystemParams) {
    params.active_effect_requirement_sync.clear();
    let mut seen_states = Vec::new();
    let mut transitions = Vec::new();
    loop {
        let state = requirement_state_signature(
            &mut params.active_effect_query,
            &params.tag_container_query,
        );
        if let Some((_, transition_start)) = seen_states
            .iter()
            .find(|(seen_state, _)| seen_state == &state)
        {
            let cycle_handles = transitions[*transition_start..].to_vec();
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
        );
        if changed_handles.is_empty() {
            return;
        }
        transitions.extend(changed_handles);
    }
}

fn remove_nonconverging_active_effects(
    mut handles: Vec<ActiveEffectHandle>,
    params: &mut AbilitySystemParams,
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

pub(crate) fn resolve_active_effect_tag_requirements_if_dirty(params: &mut AbilitySystemParams) {
    if params.active_effect_requirement_sync.take_dirty() {
        resolve_active_effect_tag_requirements(params);
    }
}

/// Bevy system wrapper for [`resolve_active_effect_tag_requirements`].
pub fn update_active_effect_tag_requirements_system(mut params: AbilitySystemParams) {
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
        for handle in handles {
            let Some(snapshot) = active_effects.get(handle).cloned() else {
                continue;
            };
            let decision = if should_remove_active_effect(&snapshot, tag_query) {
                Some(ActiveEffectRequirementDecision::Remove)
            } else {
                match (
                    passes_ongoing_requirements(&snapshot, tag_query),
                    snapshot.is_inhibited(),
                ) {
                    (false, false) => Some(ActiveEffectRequirementDecision::Inhibit),
                    (true, true) => Some(ActiveEffectRequirementDecision::Uninhibit),
                    _ => None,
                }
            };
            if let Some(decision) = decision {
                decisions.push((handle, snapshot, decision));
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

fn find_stackable_active_effect(
    source: Entity,
    target: Entity,
    spec: &GameplayEffectSpec,
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
    ignored_handles: &[ActiveEffectHandle],
) -> Option<(ActiveEffectHandle, u32)> {
    let stacking_type = spec.get_stacking_policy().get_stacking_type();
    if matches!(stacking_type, StackingType::None) {
        return None;
    }
    let active_effects = active_effect_query.get(target).ok()?;
    active_effects.handles(target).find_map(|handle| {
        if ignored_handles.contains(&handle) {
            return None;
        }
        let effect = active_effects.get(handle)?;
        (spec.is_same_def(effect.get_spec())
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
    let Ok(active_effects) = params.active_effect_query.get(target) else {
        return Ok(false);
    };
    for handle in active_effects.handles(target) {
        let Some(effect) = active_effects.get(handle) else {
            continue;
        };
        if effect.is_inhibited() {
            continue;
        }
        if effect
            .get_spec()
            .get_def_tags()
            .get_granted_application_immunity()
            .iter()
            .any(|immunity| immunity.matches_tag_bits(source_tags, Some(&incoming_asset_bits)))
        {
            return Ok(true);
        }
    }
    Ok(false)
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
    if let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) {
        attributes.remove_modifiers_for_attributes(
            attribute_id_manager,
            handle,
            effect.get_spec().get_modified_attribute_ids(),
        )?;
    }
    if let Ok(mut tags) = tag_query.get_mut(effect.get_target()) {
        tags.remove_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )?;
    }
    Ok(())
}

fn uninhibit_active_effect(
    handle: ActiveEffectHandle,
    effect: &ActiveGameplayEffect,
    attribute_id_manager: &AttributeIdManager,
    attr_query: &mut Query<&mut AttributeSet>,
    tag_query: &mut Query<&mut GameplayTagContainer>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<(), GameplayEffectApplicationError> {
    if effect.period.is_none() && !effect.get_spec().get_modifier_specs().is_empty() {
        let Ok(mut attributes) = attr_query.get_mut(effect.get_target()) else {
            return Err(GameplayEffectApplicationError::MissingAttributeSet {
                target: effect.get_target(),
            });
        };
        apply_duration_modifiers(
            effect.get_target(),
            &mut attributes,
            attribute_id_manager,
            effect.get_spec(),
            handle,
            effect.get_stack_count(),
        )?;
    }
    if !effect
        .get_spec()
        .get_def_tags()
        .get_granted_tags()
        .is_empty()
    {
        let Ok(mut tags) = tag_query.get_mut(effect.get_target()) else {
            return Err(GameplayEffectApplicationError::MissingTagContainer {
                target: effect.get_target(),
            });
        };
        tags.add_tags(
            effect.get_spec().get_def_tags().get_granted_tags(),
            tag_manager,
        )?;
    }
    Ok(())
}

fn collect_active_effects_with_tags_for_params(
    target: Entity,
    tags: &[GameplayTag],
    active_effect_query: &Query<&mut ActiveGameplayEffects>,
    tag_manager: &Res<GameplayTagManager>,
) -> Result<Vec<ActiveEffectHandle>, GameplayTagError> {
    if tags.is_empty() {
        return Ok(Vec::new());
    }
    let Ok(active_effects) = active_effect_query.get(target) else {
        return Ok(Vec::new());
    };
    let mut matches = Vec::new();
    for handle in active_effects.handles(target) {
        let Some(effect) = active_effects.get(handle) else {
            continue;
        };
        if active_effect_has_any_tags(effect, tags, tag_manager)? {
            matches.push(handle);
        }
    }
    Ok(matches)
}

fn remove_collected_active_effects_for_params(
    handles: &[ActiveEffectHandle],
    params: &mut AbilitySystemParams,
) -> Result<(), GameplayEffectApplicationError> {
    let effects: Vec<_> = handles
        .iter()
        .filter_map(|&handle| {
            params
                .active_effect_query
                .get(handle.get_target())
                .ok()
                .and_then(|active_effects| active_effects.get(handle).cloned())
                .map(|effect| (handle, effect))
        })
        .collect();
    for (_, effect) in &effects {
        validate_effect_cleanup(
            effect,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
        )?;
    }
    for (handle, effect) in effects {
        cleanup_effect_state(
            handle,
            &effect,
            EffectCleanupResources {
                attribute_id_manager: &params.attribute_id_manager,
                tag_manager: &params.tag_manager,
            },
            &mut params.attr_set_query,
            &mut params.tag_container_query,
        )?;
        if let Ok(mut active_effects) = params.active_effect_query.get_mut(handle.get_target()) {
            active_effects.remove(handle);
        }
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
    for modifier in spec.get_modifier_specs() {
        let stacked = modifier.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_duration_modifier(attribute_id_manager, &stacked, handle)
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
    for modifier in spec.get_modifier_specs() {
        let stacked = modifier.scaled_by_stack(stack_multiplier);
        attr_set
            .apply_instant_modifier(attribute_id_manager, &stacked)
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
        .any(|(effect_bits, query_bits)| (effect_bits & query_bits) != 0))
}
