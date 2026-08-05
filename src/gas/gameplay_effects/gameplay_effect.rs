//! Gameplay-effect definitions, context, tags, timing, and stacking policies.

//! Immutable gameplay-effect definitions and their evaluation inputs.

mod context;
mod definition;
mod effect_tags;
mod stacking;
mod timing;

use super::gameplay_effect_spec::{
    EffectDurationTicksSpec, EffectPeriodTicksSpec, GameplayEffectSpec,
};

pub use context::{EffectContext, EffectPayload};
pub use definition::GameplayEffect;
pub use effect_tags::{EffectTags, GameplayEffectImmunityQuery, TagRequirements};
pub use stacking::{
    StackDurationPolicy, StackExpirationPolicy, StackMagnitudePolicy, StackOverflowPolicy,
    StackPeriodPolicy, StackingPolicy, StackingType,
};
pub use timing::{EffectDurationTicks, EffectPeriodTicks};
