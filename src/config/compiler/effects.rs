//! Shared effect definitions and modifier construction.

use super::magnitude::LinearLevelMagnitude;
use super::numeric::{formula_parameters_are_finite, probability_is_valid};
use super::registration::resolve_tags;
use super::{ConfigError, EffectId, Tables, data};
use crate::{
    AttributeId, EffectDurationTicks, EffectPeriodTicks, EffectTags, GameplayEffect, GameplayTag,
    Modifier, ModifierMagnitude, ModifierOperation, StackingPolicy,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn resolve_effect(
    id: i32,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
) -> Result<Arc<GameplayEffect>, ConfigError> {
    effects
        .get(&EffectId(id))
        .map(Arc::clone)
        .ok_or_else(|| ConfigError::new(format!("Effect[{id}]"), "unresolved effect reference"))
}

fn positive_effect_ticks(ticks: i32, context: String) -> Result<f32, ConfigError> {
    let value = ticks as f32;
    if ticks <= 0 || f64::from(value) != f64::from(ticks) {
        return Err(ConfigError::new(
            context,
            "ticks must be positive and exactly representable as f32",
        ));
    }
    Ok(value)
}

pub(super) fn compile_effects(
    tables: &Tables,
    tags: &BTreeMap<String, GameplayTag>,
    attributes: &BTreeMap<String, AttributeId>,
) -> Result<BTreeMap<EffectId, Arc<GameplayEffect>>, ConfigError> {
    let mut effects = BTreeMap::new();
    for row in tables.tb_effect.iter() {
        if !probability_is_valid(row.probability) {
            return Err(ConfigError::new(
                format!("Effect[{}].probability", row.id),
                "probability must be finite and in 0..=1",
            ));
        }
        let mut rows: Vec<_> = tables
            .tb_modifier
            .iter()
            .filter(|modifier| modifier.effect_id == row.id)
            .collect();
        rows.sort_by_key(|modifier| modifier.order);
        let mut modifiers = Vec::with_capacity(rows.len());
        for modifier in rows {
            let id = attributes
                .get(&modifier.attribute)
                .copied()
                .ok_or_else(|| {
                    ConfigError::new(
                        format!("Modifier[{}].attribute", modifier.id),
                        "unresolved attribute",
                    )
                })?;
            let operation = match modifier.operation {
                data::ModifierOperation::Add => ModifierOperation::Add,
                data::ModifierOperation::PercentAdd => ModifierOperation::PercentAdd,
                data::ModifierOperation::Multiply => ModifierOperation::Multiply,
                data::ModifierOperation::Override => ModifierOperation::Override,
            };
            if !formula_parameters_are_finite(
                modifier.base,
                (modifier.magnitude_kind == data::MagnitudeKind::LinearLevel)
                    .then_some(modifier.per_level),
            ) {
                return Err(ConfigError::new(
                    format!("Modifier[{}].magnitude", modifier.id),
                    "used formula parameters must be finite",
                ));
            }
            let magnitude = match modifier.magnitude_kind {
                data::MagnitudeKind::Flat => ModifierMagnitude::Flat(modifier.base),
                data::MagnitudeKind::LinearLevel => {
                    ModifierMagnitude::Calculated(Box::new(LinearLevelMagnitude {
                        base: modifier.base,
                        per_level: modifier.per_level,
                    }))
                }
            };
            modifiers.push(Modifier::new(id, operation, magnitude));
        }
        let duration = match row.duration_kind {
            data::DurationKind::Instant => EffectDurationTicks::Instant,
            data::DurationKind::Infinite => EffectDurationTicks::Infinite,
            data::DurationKind::DurationTicks => {
                let context = format!("Effect[{}].duration_ticks", row.id);
                let ticks = row
                    .duration_ticks
                    .ok_or_else(|| ConfigError::new(&context, "missing duration"))?;
                EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(positive_effect_ticks(
                    ticks, context,
                )?))
            }
        };
        let period = row
            .period_ticks
            .map(|ticks| {
                let ticks =
                    positive_effect_ticks(ticks, format!("Effect[{}].period_ticks", row.id))?;
                Ok::<_, ConfigError>(EffectPeriodTicks::new(
                    ModifierMagnitude::Flat(ticks),
                    row.execute_on_applied,
                ))
            })
            .transpose()?;
        let effect_tags = EffectTags::new(
            resolve_tags(&row.asset_tags, tags)?,
            resolve_tags(&row.granted_tags, tags)?,
        );
        effects.insert(
            EffectId(row.id),
            Arc::new(GameplayEffect::new(
                modifiers,
                duration,
                period,
                row.probability,
                StackingPolicy::non_stacking(),
                effect_tags,
            )),
        );
    }
    Ok(effects)
}
