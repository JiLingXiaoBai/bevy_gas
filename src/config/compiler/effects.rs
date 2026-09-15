//! Shared effect definitions and modifier construction.

use super::magnitude::LinearLevelMagnitude;
use super::numeric::probability_is_valid;
use super::registration::resolve_tags;
use super::{
    ConfigError, ConfigErrorKind, ConfigLocation, EffectId, PreparedMagnitude, PreparedTables, data,
};
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
    effects.get(&EffectId(id)).map(Arc::clone).ok_or_else(|| {
        ConfigError::new(
            ConfigErrorKind::Reference,
            ConfigLocation::table("Effect").row(id),
            "unresolved effect reference",
        )
    })
}

fn positive_effect_ticks(ticks: i32, context: ConfigLocation) -> Result<f32, ConfigError> {
    let value = ticks as f32;
    if ticks <= 0 || f64::from(value) != f64::from(ticks) {
        return Err(ConfigError::new(
            ConfigErrorKind::InvalidValue,
            context,
            "ticks must be positive and exactly representable as f32",
        ));
    }
    Ok(value)
}

pub(super) fn compile_effects(
    prepared: &PreparedTables,
    tags: &BTreeMap<String, GameplayTag>,
    attributes: &BTreeMap<String, AttributeId>,
) -> Result<BTreeMap<EffectId, Arc<GameplayEffect>>, ConfigError> {
    let mut effects = BTreeMap::new();
    for row in prepared.tables.tb_effect.iter() {
        if !probability_is_valid(row.probability) {
            return Err(ConfigError::new(
                ConfigErrorKind::InvalidValue,
                ConfigLocation::table("Effect")
                    .row(row.id)
                    .field("probability"),
                "probability must be finite and in 0..=1",
            ));
        }
        let rows = prepared.modifiers(row.id);
        let mut modifiers = Vec::with_capacity(rows.len());
        for prepared_modifier in rows {
            let modifier = prepared_modifier.row;
            let id = attributes
                .get(&modifier.attribute)
                .copied()
                .ok_or_else(|| {
                    ConfigError::new(
                        ConfigErrorKind::Reference,
                        ConfigLocation::table("Modifier")
                            .row(modifier.id)
                            .field("attribute"),
                        "unresolved attribute",
                    )
                })?;
            let operation = match modifier.operation {
                data::ModifierOperation::Add => ModifierOperation::Add,
                data::ModifierOperation::PercentAdd => ModifierOperation::PercentAdd,
                data::ModifierOperation::Multiply => ModifierOperation::Multiply,
                data::ModifierOperation::Override => ModifierOperation::Override,
            };
            let magnitude = match prepared_modifier.magnitude {
                PreparedMagnitude::Flat(value) => ModifierMagnitude::Flat(value),
                PreparedMagnitude::LinearLevel { base, per_level } => {
                    ModifierMagnitude::Calculated(Box::new(LinearLevelMagnitude {
                        base,
                        per_level,
                    }))
                }
            };
            modifiers.push(Modifier::new(id, operation, magnitude));
        }
        let duration = match row.duration_kind {
            data::DurationKind::Instant => EffectDurationTicks::Instant,
            data::DurationKind::Infinite => EffectDurationTicks::Infinite,
            data::DurationKind::DurationTicks => {
                let context = ConfigLocation::table("Effect")
                    .row(row.id)
                    .field("duration_ticks");
                let ticks = row.duration_ticks.ok_or_else(|| {
                    ConfigError::new(ConfigErrorKind::InvalidValue, &context, "missing duration")
                })?;
                EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(positive_effect_ticks(
                    ticks, context,
                )?))
            }
        };
        let period = row
            .period_ticks
            .map(|ticks| {
                let ticks = positive_effect_ticks(
                    ticks,
                    ConfigLocation::table("Effect")
                        .row(row.id)
                        .field("period_ticks"),
                )?;
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
