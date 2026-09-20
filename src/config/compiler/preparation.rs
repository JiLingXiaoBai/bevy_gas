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

/// One authored action with its checked tick, parent-list position, and resolved payload.
pub(crate) struct PreparedAction {
    pub(crate) tick: u32,
    pub(crate) order: usize,
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
    actions: BTreeMap<i32, Vec<PreparedAction>>,
    additional_costs: BTreeMap<i32, Vec<PreparedAdditionalCost<'a>>>,
    modifiers: BTreeMap<i32, Vec<PreparedModifier<'a>>>,
}

impl<'a> PreparedTables<'a> {
    pub(crate) fn new(tables: &'a Tables) -> Result<Self, ConfigError> {
        let mut prepared = Self {
            tables,
            actions: BTreeMap::new(),
            additional_costs: BTreeMap::new(),
            modifiers: BTreeMap::new(),
        };
        for ability in tables.tb_ability.iter() {
            prepared
                .actions
                .insert(ability.id, prepare_actions(tables, ability)?);
            prepared
                .additional_costs
                .insert(ability.id, prepare_additional_costs(tables, ability)?);
        }
        for effect in tables.tb_effect.iter() {
            prepared
                .modifiers
                .insert(effect.id, prepare_modifiers(tables, effect)?);
        }
        Ok(prepared)
    }

    pub(crate) fn actions(&self, ability_id: i32) -> &[PreparedAction] {
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

fn resolve_reference<'a, T>(
    id: i32,
    row: Option<&'a T>,
    seen: &mut BTreeSet<i32>,
    location: &ConfigLocation,
) -> Result<&'a T, ConfigError> {
    if !seen.insert(id) {
        return Err(ConfigError::new(
            ConfigErrorKind::InvalidValue,
            location,
            format!("duplicate referenced ID {id}"),
        ));
    }
    row.ok_or_else(|| {
        ConfigError::new(
            ConfigErrorKind::Reference,
            location,
            format!("unknown referenced ID {id}"),
        )
    })
}

fn prepare_actions(
    tables: &Tables,
    ability: &data::Ability,
) -> Result<Vec<PreparedAction>, ConfigError> {
    let mut actions = Vec::with_capacity(ability.task_ids.len());
    let mut seen = BTreeSet::new();
    let references = ConfigLocation::table("Ability")
        .row(ability.id)
        .field("task_ids");
    for (order, &id) in ability.task_ids.iter().enumerate() {
        let row = resolve_reference(id, tables.tb_ability_task.get(&id), &mut seen, &references)?;
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
        actions.push(PreparedAction { tick, order, kind });
    }
    actions.sort_by_key(|action| (action.tick, action.order));
    Ok(actions)
}

fn prepare_additional_costs<'a>(
    tables: &'a Tables,
    ability: &data::Ability,
) -> Result<Vec<PreparedAdditionalCost<'a>>, ConfigError> {
    let mut costs = Vec::with_capacity(ability.additional_cost_ids.len());
    let mut seen = BTreeSet::new();
    let mut totals: BTreeMap<&str, u32> = BTreeMap::new();
    let references = ConfigLocation::table("Ability")
        .row(ability.id)
        .field("additional_cost_ids");
    for &id in &ability.additional_cost_ids {
        let row = resolve_reference(
            id,
            tables.tb_ability_additional_cost.get(&id),
            &mut seen,
            &references,
        )?;
        let location = ConfigLocation::table("AbilityAdditionalCost").row(row.id);
        if row.resource.trim().is_empty() {
            return Err(ConfigError::new(
                ConfigErrorKind::InvalidValue,
                location.field("resource"),
                "resource must not be blank",
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
        let total = totals.entry(row.resource.as_str()).or_default();
        *total = total.checked_add(amount).ok_or_else(|| {
            ConfigError::new(
                ConfigErrorKind::InvalidValue,
                location.field("amount"),
                "total amount for this ability and resource exceeds u32 capacity",
            )
        })?;
        costs.push(PreparedAdditionalCost { row, amount });
    }
    Ok(costs)
}

fn prepare_modifiers<'a>(
    tables: &'a Tables,
    effect: &data::Effect,
) -> Result<Vec<PreparedModifier<'a>>, ConfigError> {
    let mut modifiers = Vec::with_capacity(effect.modifier_ids.len());
    let mut seen = BTreeSet::new();
    let references = ConfigLocation::table("Effect")
        .row(effect.id)
        .field("modifier_ids");
    for &id in &effect.modifier_ids {
        let row = resolve_reference(id, tables.tb_modifier.get(&id), &mut seen, &references)?;
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
        modifiers.push(PreparedModifier { row, magnitude });
    }
    Ok(modifiers)
}
