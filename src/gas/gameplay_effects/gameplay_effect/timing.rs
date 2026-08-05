use super::{EffectContext, EffectDurationTicksSpec, EffectPeriodTicksSpec};
use crate::modifiers::ModifierMagnitude;

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

fn magnitude_to_ticks(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u32::MAX as f32 {
        u32::MAX
    } else {
        value.ceil() as u32
    }
}
