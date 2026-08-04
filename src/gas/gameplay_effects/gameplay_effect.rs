use super::gameplay_effect_spec::{
    EffectDurationTicksSpec, EffectPeriodTicksSpec, GameplayEffectSpec,
};
use crate::ability_system::AbilitySystemComponent;
use crate::attributes::{AttributeIdManager, AttributeSet, AttributeSetSnapshot};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagBits, GameplayTagContainer, GameplayTagError, GameplayTagManager,
    tag_bits_from_tags, tag_bits_from_tags_with_manager,
};
use crate::modifiers::{Modifier, ModifierMagnitude, ModifierOperation};
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Query;
use bevy::prelude::Res;
use std::sync::Arc;

pub struct EffectContext<'w, 's> {
    pub target: Option<Entity>,
    pub payload: &'w EffectPayload,
    pub attribute_id_manager: &'w AttributeIdManager,
    pub attr_set_query: &'w Query<'w, 's, &'static AttributeSet>,
    pub tag_container_query: &'w Query<'w, 's, &'static GameplayTagContainer>,
    pub asc_query: &'w Query<'w, 's, &'static AbilitySystemComponent>,
}

impl<'w, 's> EffectContext<'w, 's> {
    /// Returns the entity whose ASC, attributes, and tags provide this effect's gameplay source.
    pub fn source(&self) -> Entity {
        self.payload.get_source()
    }

    /// Returns the entity that initiated the action producing this effect.
    pub fn instigator(&self) -> Entity {
        self.payload.get_instigator()
    }

    /// Returns the optional physical entity that directly caused this effect.
    pub fn causer(&self) -> Option<Entity> {
        self.payload.get_causer()
    }

    pub fn level(&self) -> u32 {
        self.payload.get_level()
    }

    pub fn source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.payload.get_source_snapshot()
    }

    /// Returns the global mapping used to locate hot and cold attributes.
    pub fn attribute_id_manager(&self) -> &AttributeIdManager {
        self.attribute_id_manager
    }
}

#[derive(Clone)]
pub struct EffectPayload {
    source: Entity,
    instigator: Entity,
    causer: Option<Entity>,
    level: u32,
    source_snapshot: Option<AttributeSetSnapshot>,
}

impl EffectPayload {
    /// Creates effect metadata using `source` as both the gameplay source and default instigator.
    ///
    /// `causer` identifies the optional physical entity that directly caused the effect.
    pub fn new(source: Entity, causer: Option<Entity>, level: u32) -> Self {
        Self {
            source,
            instigator: source,
            causer,
            level,
            source_snapshot: None,
        }
    }

    /// Sets the entity that initiated the action producing this effect.
    pub fn with_instigator(mut self, instigator: Entity) -> Self {
        self.instigator = instigator;
        self
    }

    pub fn with_source_snapshot(mut self, source_snapshot: AttributeSetSnapshot) -> Self {
        self.source_snapshot = Some(source_snapshot);
        self
    }

    /// Returns the entity whose ASC, attributes, and tags provide this effect's gameplay source.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the entity that initiated the action producing this effect.
    pub fn get_instigator(&self) -> Entity {
        self.instigator
    }

    /// Returns the optional physical entity that directly caused this effect.
    pub fn get_causer(&self) -> Option<Entity> {
        self.causer
    }

    pub fn get_level(&self) -> u32 {
        self.level
    }

    pub fn get_source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.source_snapshot.as_ref()
    }
}

pub enum EffectDurationTicks {
    Instant,
    DurationTicks(ModifierMagnitude),
    Infinite,
}

impl EffectDurationTicks {
    pub fn make_spec(&self, context: &EffectContext) -> EffectDurationTicksSpec {
        match self {
            EffectDurationTicks::Instant => EffectDurationTicksSpec::Instant,
            EffectDurationTicks::DurationTicks(mm) => {
                EffectDurationTicksSpec::DurationTicks(magnitude_to_ticks(match mm {
                    ModifierMagnitude::Flat(f) => *f,
                    ModifierMagnitude::Calculated(mmc) => mmc.calculate(context),
                }))
            }
            EffectDurationTicks::Infinite => EffectDurationTicksSpec::Infinite,
        }
    }
}

pub struct EffectPeriodTicks {
    period_ticks: ModifierMagnitude,
    execute_on_applied: bool,
}

impl EffectPeriodTicks {
    pub fn new(period_ticks: ModifierMagnitude, execute_on_applied: bool) -> Self {
        Self {
            period_ticks,
            execute_on_applied,
        }
    }

    pub fn make_spec(&self, context: &EffectContext) -> EffectPeriodTicksSpec {
        let final_value = match &self.period_ticks {
            ModifierMagnitude::Flat(f) => *f,
            ModifierMagnitude::Calculated(mmc) => mmc.calculate(context),
        };
        let final_value = magnitude_to_ticks(final_value);
        EffectPeriodTicksSpec::new(final_value, self.execute_on_applied)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StackingType {
    None,
    AggregateBySource,
    AggregateByTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum StackMagnitudePolicy {
    None,
    Linear,
}

#[derive(Debug, Clone, Copy)]
pub enum StackDurationPolicy {
    KeepExisting,
    RefreshOnSuccessfulStack,
}

#[derive(Debug, Clone, Copy)]
pub enum StackPeriodPolicy {
    KeepCurrentTick,
    ResetOnSuccessfulStack,
}

#[derive(Debug, Clone, Copy)]
pub enum StackOverflowPolicy {
    RejectApplication,
    RefreshDuration,
}

#[derive(Debug, Clone, Copy)]
pub enum StackExpirationPolicy {
    RemoveAllStacks,
    RemoveSingleStack,
}

#[derive(Debug, Clone, Copy)]
pub struct StackingPolicy {
    stacking_type: StackingType,
    stack_limit: u32,
    magnitude_policy: StackMagnitudePolicy,
    duration_policy: StackDurationPolicy,
    period_policy: StackPeriodPolicy,
    overflow_policy: StackOverflowPolicy,
    expiration_policy: StackExpirationPolicy,
}

impl StackingPolicy {
    pub fn new(
        stacking_type: StackingType,
        stack_limit: u32,
        magnitude_policy: StackMagnitudePolicy,
        duration_policy: StackDurationPolicy,
        period_policy: StackPeriodPolicy,
        overflow_policy: StackOverflowPolicy,
        expiration_policy: StackExpirationPolicy,
    ) -> Self {
        Self {
            stacking_type,
            stack_limit,
            magnitude_policy,
            duration_policy,
            period_policy,
            overflow_policy,
            expiration_policy,
        }
    }

    pub fn non_stacking() -> Self {
        Self::new(
            StackingType::None,
            0,
            StackMagnitudePolicy::None,
            StackDurationPolicy::KeepExisting,
            StackPeriodPolicy::KeepCurrentTick,
            StackOverflowPolicy::RejectApplication,
            StackExpirationPolicy::RemoveAllStacks,
        )
    }

    pub fn linear_refreshing(stacking_type: StackingType, stack_limit: u32) -> Self {
        Self::new(
            stacking_type,
            stack_limit,
            StackMagnitudePolicy::Linear,
            StackDurationPolicy::RefreshOnSuccessfulStack,
            StackPeriodPolicy::ResetOnSuccessfulStack,
            StackOverflowPolicy::RejectApplication,
            StackExpirationPolicy::RemoveAllStacks,
        )
    }

    pub fn get_stacking_type(&self) -> StackingType {
        self.stacking_type
    }

    pub fn get_stack_limit(&self) -> u32 {
        self.stack_limit
    }

    pub fn get_magnitude_policy(&self) -> StackMagnitudePolicy {
        self.magnitude_policy
    }

    pub fn get_duration_policy(&self) -> StackDurationPolicy {
        self.duration_policy
    }

    pub fn get_period_policy(&self) -> StackPeriodPolicy {
        self.period_policy
    }

    pub fn get_overflow_policy(&self) -> StackOverflowPolicy {
        self.overflow_policy
    }

    pub fn get_expiration_policy(&self) -> StackExpirationPolicy {
        self.expiration_policy
    }
}

#[derive(Default)]
pub struct TagRequirements {
    require_all: Vec<GameplayTag>,
    ignore_any: Vec<GameplayTag>,
    require_all_bits: GameplayTagBits,
    ignore_any_bits: GameplayTagBits,
}

impl TagRequirements {
    /// Creates and precomputes a set of required and ignored tags.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if any tag index exceeds
    /// the configured capacity.
    pub fn new(
        require_all: Vec<GameplayTag>,
        ignore_any: Vec<GameplayTag>,
    ) -> Result<Self, GameplayTagError> {
        let require_all_bits = tag_bits_from_tags(&require_all)?;
        let ignore_any_bits = tag_bits_from_tags(&ignore_any)?;
        Ok(Self {
            require_all,
            ignore_any,
            require_all_bits,
            ignore_any_bits,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.require_all.is_empty() && self.ignore_any.is_empty()
    }

    pub fn passes(&self, tags: Option<&GameplayTagContainer>) -> bool {
        if self.is_empty() {
            return true;
        }

        let Some(tags) = tags else {
            return false;
        };

        let has_required = tags.has_all_bits(&self.require_all_bits);
        let has_blocked = tags.has_any_bits(&self.ignore_any_bits);

        has_required && !has_blocked
    }

    /// Tests an ordinary tag slice after expanding its inherited tags.
    ///
    /// # Errors
    ///
    /// Returns [`GameplayTagError::InvalidTagIndex`] if a tag is not registered
    /// in `tag_manager`.
    pub fn passes_tag_slice(
        &self,
        tags: &[GameplayTag],
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError> {
        if self.is_empty() {
            return Ok(true);
        }

        let tag_bits = tag_bits_from_tags_with_manager(tags, tag_manager)?;

        Ok(self.passes_tag_bits(&tag_bits))
    }

    pub fn passes_tag_bits(&self, tag_bits: &GameplayTagBits) -> bool {
        if self.is_empty() {
            return true;
        }

        let has_required = tag_bits
            .iter()
            .zip(self.require_all_bits.iter())
            .all(|(a, b)| (a & b) == *b);
        let has_blocked = tag_bits
            .iter()
            .zip(self.ignore_any_bits.iter())
            .any(|(a, b)| (a & b) != 0);

        has_required && !has_blocked
    }

    pub fn get_required_tags(&self) -> &[GameplayTag] {
        &self.require_all
    }

    pub fn get_ignored_tags(&self) -> &[GameplayTag] {
        &self.ignore_any
    }
}

#[derive(Default)]
pub struct GameplayEffectImmunityQuery {
    source_tags: TagRequirements,
    effect_tags: TagRequirements,
}

impl GameplayEffectImmunityQuery {
    pub fn new(source_tags: TagRequirements, effect_tags: TagRequirements) -> Self {
        Self {
            source_tags,
            effect_tags,
        }
    }

    pub fn matches(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_tags: &[GameplayTag],
        tag_manager: &Res<GameplayTagManager>,
    ) -> Result<bool, GameplayTagError> {
        Ok(self.source_tags.passes(source_tags)
            && self
                .effect_tags
                .passes_tag_slice(effect_asset_tags, tag_manager)?)
    }

    pub fn matches_tag_bits(
        &self,
        source_tags: Option<&GameplayTagContainer>,
        effect_asset_bits: Option<&GameplayTagBits>,
    ) -> bool {
        self.source_tags.passes(source_tags)
            && effect_asset_bits.is_some_and(|bits| self.effect_tags.passes_tag_bits(bits))
    }
}

pub struct EffectTags {
    asset_tags: Vec<GameplayTag>,
    granted_tags: Vec<GameplayTag>,
    source_application_tags: TagRequirements,
    target_application_tags: TagRequirements,
    source_ongoing_tags: TagRequirements,
    target_ongoing_tags: TagRequirements,
    source_removal_tags: TagRequirements,
    target_removal_tags: TagRequirements,
    granted_application_immunity: Vec<GameplayEffectImmunityQuery>,
    remove_effects_with_tags: Vec<GameplayTag>,
}

impl EffectTags {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset_tags: Vec<GameplayTag>,
        granted_tags: Vec<GameplayTag>,
        source_application_tags: TagRequirements,
        target_application_tags: TagRequirements,
        source_ongoing_tags: TagRequirements,
        target_ongoing_tags: TagRequirements,
        source_removal_tags: TagRequirements,
        target_removal_tags: TagRequirements,
        granted_application_immunity: Vec<GameplayEffectImmunityQuery>,
        remove_effects_with_tags: Vec<GameplayTag>,
    ) -> Self {
        Self {
            asset_tags,
            granted_tags,
            source_application_tags,
            target_application_tags,
            source_ongoing_tags,
            target_ongoing_tags,
            source_removal_tags,
            target_removal_tags,
            granted_application_immunity,
            remove_effects_with_tags,
        }
    }

    pub fn get_asset_tags(&self) -> &[GameplayTag] {
        &self.asset_tags
    }

    pub fn get_granted_tags(&self) -> &[GameplayTag] {
        &self.granted_tags
    }

    pub fn get_required_tags(&self) -> &[GameplayTag] {
        self.target_application_tags.get_required_tags()
    }

    pub fn get_blocked_tags(&self) -> &[GameplayTag] {
        self.target_application_tags.get_ignored_tags()
    }

    pub fn get_source_application_tags(&self) -> &TagRequirements {
        &self.source_application_tags
    }

    pub fn get_target_application_tags(&self) -> &TagRequirements {
        &self.target_application_tags
    }

    pub fn get_source_ongoing_tags(&self) -> &TagRequirements {
        &self.source_ongoing_tags
    }

    pub fn get_target_ongoing_tags(&self) -> &TagRequirements {
        &self.target_ongoing_tags
    }

    pub fn get_source_removal_tags(&self) -> &TagRequirements {
        &self.source_removal_tags
    }

    pub fn get_target_removal_tags(&self) -> &TagRequirements {
        &self.target_removal_tags
    }

    pub fn get_granted_application_immunity(&self) -> &[GameplayEffectImmunityQuery] {
        &self.granted_application_immunity
    }

    pub fn get_remove_effects_with_tags(&self) -> &[GameplayTag] {
        &self.remove_effects_with_tags
    }
}

// stored as a Resource
pub struct GameplayEffect {
    modifiers: Vec<Modifier>,
    duration: EffectDurationTicks,
    period: Option<EffectPeriodTicks>,
    probability_to_apply: f32,
    stacking_policy: StackingPolicy,
    tags: EffectTags,
}

impl GameplayEffect {
    pub fn new(
        modifiers: Vec<Modifier>,
        duration: EffectDurationTicks,
        period: Option<EffectPeriodTicks>,
        probability_to_apply: f32,
        stacking_policy: StackingPolicy,
        tags: EffectTags,
    ) -> Self {
        Self {
            modifiers,
            duration,
            period,
            probability_to_apply,
            stacking_policy,
            tags,
        }
    }

    pub fn make_spec(self: &Arc<Self>, context: &EffectContext) -> GameplayEffectSpec {
        GameplayEffectSpec::new(
            self.clone(),
            self.modifiers
                .iter()
                .map(|m| m.make_spec(context))
                .collect(),
            self.duration.make_spec(context),
            self.period.as_ref().map(|p| p.make_spec(context)),
            self.stacking_policy,
        )
    }

    pub fn get_tags(&self) -> &EffectTags {
        &self.tags
    }

    pub fn has_only_add_modifiers(&self) -> bool {
        self.modifiers
            .iter()
            .all(|modifier| modifier.get_operation() == ModifierOperation::Add)
    }

    pub fn get_probability_to_apply(&self) -> f32 {
        self.probability_to_apply
    }
}

fn magnitude_to_ticks(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u32::MAX as f32 {
        u32::MAX
    } else {
        value.ceil() as u32
    }
}
