use super::{
    EffectContext, EffectDurationTicks, EffectPeriodTicks, EffectTags, GameplayEffectSpec,
    StackingPolicy,
};
use crate::modifiers::{Modifier, ModifierOperation};
use std::sync::Arc;

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

    /// Returns the stacking behavior configured for this effect definition.
    pub fn get_stacking_policy(&self) -> StackingPolicy {
        self.stacking_policy
    }
}
