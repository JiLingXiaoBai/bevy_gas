use super::{
    EffectContext, EffectDurationTicks, EffectPeriodTicks, EffectTags, GameplayEffectSpec,
    StackingPolicy,
};
use crate::modifiers::{Modifier, ModifierOperation};
use std::sync::Arc;

/// Defines shared effect formulas, timing, stacking, and tag rules.
///
/// Definitions are shared through [`Arc`], rather than stored as ECS resources themselves.
/// [`Self::make_spec`] evaluates formulas for one application; the resulting specification
/// stores those values separately from the runtime lifetime and stack state of an active effect.
pub struct GameplayEffect {
    modifiers: Vec<Modifier>,
    duration: EffectDurationTicks,
    period: Option<EffectPeriodTicks>,
    probability_to_apply: f32,
    stacking_policy: StackingPolicy,
    tags: EffectTags,
}

impl GameplayEffect {
    /// Creates an effect definition without applying it or evaluating its formulas.
    ///
    /// `modifiers` describe attribute changes, `duration` selects instant or persistent
    /// behavior, and `period` optionally schedules repeated execution in fixed ticks.
    /// `probability_to_apply` configures the application chance, `stacking_policy` controls
    /// repeated applications, and `tags` configure identity and lifecycle rules.
    /// Returns the definition; application performs runtime validation.
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

    /// Evaluates this definition using `context` and returns a specification sharing its [`Arc`].
    ///
    /// Modifier magnitudes, duration, and period formulas are evaluated once here. Periodic
    /// execution reuses the captured values rather than recalculating them from live attributes.
    /// This method only creates the specification; it does not check application requirements
    /// or apply changes to an entity.
    pub fn make_spec(self: &Arc<Self>, context: &EffectContext) -> GameplayEffectSpec {
        GameplayEffectSpec::new(
            self.clone(),
            self.modifiers
                .iter()
                .map(|m| m.make_spec(context))
                .collect(),
            self.duration.make_spec(context),
            self.period.as_ref().map(|p| p.make_spec(context)),
        )
    }

    /// Returns the effect's identity, granted tags, and lifecycle rules.
    pub fn get_tags(&self) -> &EffectTags {
        &self.tags
    }

    /// Returns whether every modifier uses addition, including when there are no modifiers.
    ///
    /// Ability costs require additive modifiers so affordability can be checked before payment.
    pub fn has_only_add_modifiers(&self) -> bool {
        self.modifiers
            .iter()
            .all(|modifier| modifier.get_operation() == ModifierOperation::Add)
    }

    /// Returns the configured application probability without sampling it.
    pub fn get_probability_to_apply(&self) -> f32 {
        self.probability_to_apply
    }

    /// Returns the stacking behavior configured for this effect definition.
    pub fn get_stacking_policy(&self) -> StackingPolicy {
        self.stacking_policy
    }
}
