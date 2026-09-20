//! Borrowed rows indexed and normalized once for compilation, validation, and reports.

#[cfg(feature = "config-validation")]
use super::magnitude::evaluate_linear;
use super::numeric::formula_parameters_are_finite;
use super::{ConfigError, ConfigErrorKind, ConfigLocation, Tables, data};
use std::collections::{BTreeMap, BTreeSet};

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
    pub(crate) row: &'a data::AbilityTask,
    pub(crate) tick: u32,
    pub(crate) kind: PreparedActionKind,
}

/// One external-resource requirement with a checked runtime quantity.
pub(crate) struct PreparedAdditionalCost<'a> {
    pub(crate) row: &'a data::AbilityAdditionalCost,
    pub(crate) amount: u32,
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
    additional_costs: BTreeMap<i32, Vec<PreparedAdditionalCost<'a>>>,
    modifiers: BTreeMap<i32, Vec<PreparedModifier<'a>>>,
}

impl<'a> PreparedTables<'a> {
    pub(crate) fn new(tables: &'a Tables) -> Result<Self, ConfigError> {
        let mut prepared = Self {
            tables,
            actions: BTreeMap::new(),
            additional_costs: prepare_additional_costs(tables)?,
            modifiers: BTreeMap::new(),
        };
        for row in tables.tb_ability_task.iter() {
            // Orphan rows remain authoring errors; trusted runtime tables ignore unused rows.
            if tables.tb_ability.get(&row.ability_id).is_none() {
                continue;
            }
            let location = ConfigLocation::table("AbilityTask").row(row.id);
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

    pub(crate) fn additional_costs(&self, ability_id: i32) -> &[PreparedAdditionalCost<'a>] {
        self.additional_costs
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

fn prepare_additional_costs(
    tables: &Tables,
) -> Result<BTreeMap<i32, Vec<PreparedAdditionalCost<'_>>>, ConfigError> {
    let mut costs: BTreeMap<i32, Vec<PreparedAdditionalCost<'_>>> = BTreeMap::new();
    let mut positions = BTreeSet::new();
    let mut totals: BTreeMap<(i32, &str), u32> = BTreeMap::new();
    for row in tables.tb_ability_additional_cost.iter() {
        let location = ConfigLocation::table("AbilityAdditionalCost").row(row.id);
        // Ignoring an orphan requirement could silently make an authored ability free.
        if tables.tb_ability.get(&row.ability_id).is_none() {
            return Err(ConfigError::new(
                ConfigErrorKind::Reference,
                location.field("ability_id"),
                "unknown ability",
            ));
        }
        if row.resource.trim().is_empty() {
            return Err(ConfigError::new(
                ConfigErrorKind::InvalidValue,
                location.field("resource"),
                "resource must not be blank",
            ));
        }
        if row.order < 0 || !positions.insert((row.ability_id, row.order)) {
            return Err(ConfigError::new(
                ConfigErrorKind::InvalidValue,
                location.field("order"),
                "order must be nonnegative and unique within the ability",
            ));
        }
        let amount = u32::try_from(row.amount)
            .ok()
            .filter(|amount| *amount > 0)
            .ok_or_else(|| {
                ConfigError::new(
                    ConfigErrorKind::InvalidValue,
                    location.field("amount"),
                    "amount must be in 1..=4294967295",
                )
            })?;
        let total = totals.entry((row.ability_id, &row.resource)).or_default();
        *total = total.checked_add(amount).ok_or_else(|| {
            ConfigError::new(
                ConfigErrorKind::InvalidValue,
                location.field("amount"),
                "total amount for this ability and resource exceeds u32 capacity",
            )
        })?;
        costs
            .entry(row.ability_id)
            .or_default()
            .push(PreparedAdditionalCost { row, amount });
    }
    for costs in costs.values_mut() {
        costs.sort_by_key(|cost| cost.row.order);
    }
    Ok(costs)
}
