use super::{
    AbilityId, ConfigError, ConfigErrorKind, ConfigLocation, PreparedActionKind, PreparedTables,
    Tables, validate_prepared,
};
use std::fmt::{self, Write};

/// Describes ability `id` at level one after validating the supplied `tables`.
///
/// Returns the resolved effects, external costs, targeting rules, and ordered timeline, or an
/// error when the tables are invalid or the requested ability does not exist.
pub fn describe_ability(tables: &Tables, id: AbilityId) -> Result<String, ConfigError> {
    describe_ability_at_level(tables, id, 1)
}

/// Describes ability `id` at `level` using the runtime compiler's magnitude formula.
///
/// Returns a report including resolved modifier values, external costs, effect timing,
/// targeting, and the ordered timeline. Invalid tables, IDs, or unsupported levels return an error.
pub fn describe_ability_at_level(
    tables: &Tables,
    id: AbilityId,
    level: u32,
) -> Result<String, ConfigError> {
    let prepared = PreparedTables::new(tables)?;
    validate_prepared(&prepared)?;
    let ability = tables.tb_ability.get(&id.0).ok_or_else(|| {
        ConfigError::new(
            ConfigErrorKind::UnknownAbility,
            ConfigLocation::table("Ability").row(id.0),
            "unknown ability",
        )
    })?;
    if level == 0 || level > ability.max_level as u32 {
        return Err(ConfigError::new(
            ConfigErrorKind::UnsupportedLevel,
            ConfigLocation::table("Ability").row(id.0).field("level"),
            format!("level {level} is outside 1..={}", ability.max_level),
        ));
    }
    let target = tables
        .tb_targeting
        .get(&ability.targeting_id)
        .ok_or_else(|| {
            ConfigError::new(
                ConfigErrorKind::Reference,
                ConfigLocation::table("Ability")
                    .row(id.0)
                    .field("targeting_id"),
                "unknown targeting",
            )
        })?;
    let mut report = String::new();
    writeln!(
        report,
        "Ability {}: {} | level {level}/{}",
        ability.id, ability.name, ability.max_level
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "Targeting {}: {} | {:?}",
        target.id, target.name, target.selection
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "  radius={:?}, half_angle_radians={:?}, max_distance={:?}, sort={:?}, limit={}",
        target.radius, target.half_angle_radians, target.max_distance, target.sort, target.limit
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "  exclude_source={}, require_attributes={}, required_tags={:?}, blocked_tags={:?}",
        target.exclude_source, target.require_attributes, target.required_tags, target.blocked_tags
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "Ability tags: {:?} | required={:?} | blocked={:?}",
        ability.asset_tags, ability.required_tags, ability.blocked_tags
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "Multiple instances: {}",
        ability.allow_multiple_instances
    )
    .map_err(report_error)?;
    writeln!(
        report,
        "Activation commits cost and cooldown to the owner and captures targets once."
    )
    .map_err(report_error)?;
    for (role, effect_id) in [
        ("Cost", ability.cost_effect_id),
        ("Cooldown", ability.cooldown_effect_id),
    ] {
        if let Some(effect_id) = effect_id {
            writeln!(report, "{role}:").map_err(report_error)?;
            append_effect(&mut report, &prepared, effect_id, level)?;
        }
    }
    for (order, cost) in prepared.additional_costs(id.0).iter().enumerate() {
        writeln!(
            report,
            "Additional cost / order {}: resource={}, amount={}",
            order, cost.row.resource, cost.amount
        )
        .map_err(report_error)?;
    }
    for action in prepared.actions(id.0) {
        match action.kind {
            PreparedActionKind::ApplyEffect { effect_id, scope } => {
                writeln!(
                    report,
                    "tick {} / order {}: ApplyEffect to {:?}",
                    action.tick, action.order, scope
                )
                .map_err(report_error)?;
                append_effect(&mut report, &prepared, effect_id, level)?;
            }
            PreparedActionKind::EndAbility => writeln!(
                report,
                "tick {} / order {}: EndAbility",
                action.tick, action.order
            )
            .map_err(report_error)?,
        }
    }
    Ok(report)
}

fn append_effect(
    report: &mut String,
    prepared: &PreparedTables,
    id: i32,
    level: u32,
) -> Result<(), ConfigError> {
    let effect = prepared.tables.tb_effect.get(&id).ok_or_else(|| {
        ConfigError::new(
            ConfigErrorKind::Reference,
            ConfigLocation::table("Effect").row(id),
            "unknown effect",
        )
    })?;
    writeln!(report, "  Effect {id}: {} | {:?}, duration_ticks={:?}, period_ticks={:?}, execute_on_applied={}, probability={}", effect.name, effect.duration_kind, effect.duration_ticks, effect.period_ticks, effect.execute_on_applied, effect.probability).map_err(report_error)?;
    writeln!(
        report,
        "  asset_tags={:?}, granted_tags={:?}",
        effect.asset_tags, effect.granted_tags
    )
    .map_err(report_error)?;
    for prepared_modifier in prepared.modifiers(id) {
        let modifier = prepared_modifier.row;
        let value = prepared_modifier.magnitude.evaluate(level) as f32;
        writeln!(
            report,
            "  {} {:?} {value} | {:?}: base={}, per_level={}",
            modifier.attribute,
            modifier.operation,
            modifier.magnitude_kind,
            modifier.base,
            modifier.per_level
        )
        .map_err(report_error)?;
    }
    Ok(())
}

fn report_error(error: fmt::Error) -> ConfigError {
    ConfigError::new(
        ConfigErrorKind::Report,
        ConfigLocation::Operation("ability report"),
        error.to_string(),
    )
}
