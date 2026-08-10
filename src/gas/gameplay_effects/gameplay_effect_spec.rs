use super::{EffectTags, GameplayEffect, StackingPolicy};
use crate::attributes::AttributeId;
use crate::modifiers::ModifierSpec;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum EffectDurationTicksSpec {
    Instant,
    DurationTicks(u32),
    Infinite,
}

impl EffectDurationTicksSpec {
    pub fn is_infinite(&self) -> bool {
        matches!(self, EffectDurationTicksSpec::Infinite)
    }

    pub fn is_instant(&self) -> bool {
        matches!(self, EffectDurationTicksSpec::Instant)
    }

    pub fn is_duration(&self) -> bool {
        matches!(self, EffectDurationTicksSpec::DurationTicks(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EffectPeriodTicksSpec {
    period_ticks: u32,
    execute_on_applied: bool,
}

impl EffectPeriodTicksSpec {
    pub fn new(period_ticks: u32, execute_on_applied: bool) -> Self {
        Self {
            period_ticks,
            execute_on_applied,
        }
    }

    pub fn get_period_ticks(&self) -> u32 {
        self.period_ticks
    }

    pub fn get_execute_on_applied(&self) -> bool {
        self.execute_on_applied
    }
}

#[derive(Clone)]
pub struct GameplayEffectSpec {
    def: Arc<GameplayEffect>,
    modifier_specs: Vec<ModifierSpec>,
    duration_spec: EffectDurationTicksSpec,
    period_spec: Option<EffectPeriodTicksSpec>,
}

impl GameplayEffectSpec {
    /// Creates an evaluated effect specification tied to its immutable definition.
    pub fn new(
        def: Arc<GameplayEffect>,
        modifier_specs: Vec<ModifierSpec>,
        duration_spec: EffectDurationTicksSpec,
        period_spec: Option<EffectPeriodTicksSpec>,
    ) -> Self {
        Self {
            def,
            modifier_specs,
            duration_spec,
            period_spec,
        }
    }

    pub fn is_same_def(&self, other: &GameplayEffectSpec) -> bool {
        Arc::ptr_eq(&self.def, &other.def)
    }

    pub fn get_def_tags(&self) -> &EffectTags {
        self.def.get_tags()
    }

    pub fn get_duration_spec(&self) -> &EffectDurationTicksSpec {
        &self.duration_spec
    }

    pub fn get_modifier_specs(&self) -> &[ModifierSpec] {
        &self.modifier_specs
    }

    pub fn get_modified_attribute_ids(&self) -> impl Iterator<Item = AttributeId> + '_ {
        self.modifier_specs.iter().map(ModifierSpec::get_id)
    }

    pub fn get_period_spec(&self) -> &Option<EffectPeriodTicksSpec> {
        &self.period_spec
    }

    /// Returns the stacking policy owned by the immutable effect definition.
    pub fn get_stacking_policy(&self) -> StackingPolicy {
        self.def.get_stacking_policy()
    }
}
