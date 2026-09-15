//! Borrowed rows indexed and normalized once for compilation, validation, and reports.

#[cfg(feature = "config-validation")]
use super::magnitude::evaluate_linear;
use super::numeric::formula_parameters_are_finite;
use super::{ConfigError, ConfigErrorKind, ConfigLocation, Tables, data};
use std::collections::BTreeMap;

/// Runtime-relevant action semantics, independent of ECS registry IDs.
#[derive(Clone, Copy)]
pub(crate) enum PreparedActionKind {
    EndAbility,
    ApplyEffect {
        effect_id: i32,
        scope: PreparedTargetScope,
    },
}

/// Supported effect recipients after resolving the raw optional target scope.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PreparedTargetScope {
    Primary,
    AllCaptured,
}

/// One authored action with its checked tick and resolved payload.
pub(crate) struct PreparedAction<'a> {
    pub(crate) row: &'a data::AbilityAction,
    pub(crate) tick: u32,
    pub(crate) kind: PreparedActionKind,
}

/// Magnitude parameters actually used by the runtime.
#[derive(Clone, Copy)]
pub(crate) enum PreparedMagnitude {
    Flat(f32),
    LinearLevel { base: f32, per_level: f32 },
}

impl PreparedMagnitude {
    #[cfg(feature = "config-validation")]
    pub(crate) fn evaluate(self, level: u32) -> f64 {
        match self {
            Self::Flat(value) => f64::from(value),
            Self::LinearLevel { base, per_level } => evaluate_linear(base, per_level, level),
        }
    }
}

pub(crate) struct PreparedModifier<'a> {
    pub(crate) row: &'a data::Modifier,
    pub(crate) magnitude: PreparedMagnitude,
}

/// A short-lived view retaining generated rows and deterministic relation indexes.
pub(crate) struct PreparedTables<'a> {
    pub(crate) tables: &'a Tables,
    actions: BTreeMap<i32, Vec<PreparedAction<'a>>>,
    modifiers: BTreeMap<i32, Vec<PreparedModifier<'a>>>,
}

impl<'a> PreparedTables<'a> {
    pub(crate) fn new(tables: &'a Tables) -> Result<Self, ConfigError> {
        let mut prepared = Self {
            tables,
            actions: BTreeMap::new(),
            modifiers: BTreeMap::new(),
        };
        for row in tables.tb_ability_action.iter() {
            // Orphan rows remain authoring errors; trusted runtime tables ignore unused rows.
            if tables.tb_ability.get(&row.ability_id).is_none() {
                continue;
            }
            let location = ConfigLocation::table("AbilityAction").row(row.id);
            let tick = u32::try_from(row.at_tick).map_err(|error| {
                ConfigError::new(
                    ConfigErrorKind::InvalidValue,
                    location.field("at_tick"),
                    error.to_string(),
                )
            })?;
            let kind = match row.kind {
                data::ActionKind::EndAbility => PreparedActionKind::EndAbility,
                data::ActionKind::ApplyEffect => {
                    let effect_id = row
                        .effect_id
                        .filter(|id| tables.tb_effect.get(id).is_some())
                        .ok_or_else(|| {
                            ConfigError::new(
                                ConfigErrorKind::Reference,
                                location.field("effect_id"),
                                "ApplyEffect requires a known effect",
                            )
                        })?;
                    let scope = match row.target_scope {
                        data::TargetScope::Primary => PreparedTargetScope::Primary,
                        data::TargetScope::AllCaptured => PreparedTargetScope::AllCaptured,
                        data::TargetScope::None => {
                            return Err(ConfigError::new(
                                ConfigErrorKind::InvalidValue,
                                location.field("target_scope"),
                                "ApplyEffect needs a target scope",
                            ));
                        }
                    };
                    PreparedActionKind::ApplyEffect { effect_id, scope }
                }
            };
            prepared
                .actions
                .entry(row.ability_id)
                .or_default()
                .push(PreparedAction { row, tick, kind });
        }
        for actions in prepared.actions.values_mut() {
            actions.sort_by_key(|action| (action.tick, action.row.order));
        }
        for row in tables.tb_modifier.iter() {
            if tables.tb_effect.get(&row.effect_id).is_none() {
                continue;
            }
            let slope =
                (row.magnitude_kind == data::MagnitudeKind::LinearLevel).then_some(row.per_level);
            if !formula_parameters_are_finite(row.base, slope) {
                return Err(ConfigError::new(
                    ConfigErrorKind::InvalidValue,
                    ConfigLocation::table("Modifier")
                        .row(row.id)
                        .field("magnitude"),
                    "used formula parameters must be finite",
                ));
            }
            let magnitude = match row.magnitude_kind {
                data::MagnitudeKind::Flat => PreparedMagnitude::Flat(row.base),
                data::MagnitudeKind::LinearLevel => PreparedMagnitude::LinearLevel {
                    base: row.base,
                    per_level: row.per_level,
                },
            };
            prepared
                .modifiers
                .entry(row.effect_id)
                .or_default()
                .push(PreparedModifier { row, magnitude });
        }
        for modifiers in prepared.modifiers.values_mut() {
            modifiers.sort_by_key(|modifier| modifier.row.order);
        }
        Ok(prepared)
    }

    pub(crate) fn actions(&self, ability_id: i32) -> &[PreparedAction<'a>] {
        self.actions
            .get(&ability_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn modifiers(&self, effect_id: i32) -> &[PreparedModifier<'a>] {
        self.modifiers
            .get(&effect_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}
