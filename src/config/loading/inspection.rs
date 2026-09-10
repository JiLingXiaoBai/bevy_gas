use super::{AbilityId, ActionKind, ConfigError, Tables, evaluate_linear, validate_tables};
use std::fmt::{self, Write};

/// Describes ability `id` at level one after validating the supplied `tables`.
///
/// Returns the resolved effects, targeting rules, and ordered timeline, or an
/// error when the tables are invalid or the requested ability does not exist.
pub fn describe_ability(tables: &Tables, id: AbilityId) -> Result<String, ConfigError> {
    describe_ability_at_level(tables, id, 1)
}

/// Describes ability `id` at `level` using the runtime compiler's magnitude formula.
///
/// Returns a report including resolved modifier values, effect timing, targeting,
/// and the ordered timeline. Invalid tables, IDs, or unsupported levels return an error.
pub fn describe_ability_at_level(
    tables: &Tables,
    id: AbilityId,
    level: u32,
) -> Result<String, ConfigError> {
    validate_tables(tables)?;
    let ability = tables
        .tb_ability
        .get(&id.0)
        .ok_or_else(|| ConfigError::new(format!("Ability[{}]", id.0), "unknown ability"))?;
    if level == 0 || level > ability.max_level as u32 {
        return Err(ConfigError::new(
            format!("Ability[{}].level", id.0),
            format!("level {level} is outside 1..={}", ability.max_level),
        ));
    }
    let target = tables
        .tb_targeting
        .get(&ability.targeting_id)
        .ok_or_else(|| {
            ConfigError::new(
                format!("Ability[{}].targeting_id", id.0),
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
        "End on activation: {} | multiple instances: {}",
        ability.end_on_activation, ability.allow_multiple_instances
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
            append_effect(&mut report, tables, effect_id, level)?;
        }
    }
    for &effect_id in &ability.activation_effect_ids {
        writeln!(report, "Activation effect to all captured targets:").map_err(report_error)?;
        append_effect(&mut report, tables, effect_id, level)?;
    }
    let mut actions: Vec<_> = tables
        .tb_ability_action
        .iter()
        .filter(|action| action.ability_id == id.0)
        .collect();
    actions.sort_by_key(|action| (action.at_tick, action.order));
    for action in actions {
        match action.kind {
            ActionKind::ApplyEffect => {
                writeln!(
                    report,
                    "tick {} / order {}: ApplyEffect to {:?}",
                    action.at_tick, action.order, action.target_scope
                )
                .map_err(report_error)?;
                let effect_id = action.effect_id.ok_or_else(|| {
                    ConfigError::new(
                        format!("AbilityAction[{}]", action.id),
                        "missing effect reference",
                    )
                })?;
                append_effect(&mut report, tables, effect_id, level)?;
            }
            ActionKind::EndAbility => writeln!(
                report,
                "tick {} / order {}: EndAbility",
                action.at_tick, action.order
            )
            .map_err(report_error)?,
        }
    }
    Ok(report)
}

fn append_effect(
    report: &mut String,
    tables: &Tables,
    id: i32,
    level: u32,
) -> Result<(), ConfigError> {
    let effect = tables
        .tb_effect
        .get(&id)
        .ok_or_else(|| ConfigError::new(format!("Effect[{id}]"), "unknown effect"))?;
    writeln!(report, "  Effect {id}: {} | {:?}, duration_ticks={:?}, period_ticks={:?}, execute_on_applied={}, probability={}", effect.name, effect.duration_kind, effect.duration_ticks, effect.period_ticks, effect.execute_on_applied, effect.probability).map_err(report_error)?;
    writeln!(
        report,
        "  asset_tags={:?}, granted_tags={:?}",
        effect.asset_tags, effect.granted_tags
    )
    .map_err(report_error)?;
    let mut modifiers: Vec<_> = tables
        .tb_modifier
        .iter()
        .filter(|modifier| modifier.effect_id == id)
        .collect();
    modifiers.sort_by_key(|modifier| modifier.order);
    for modifier in modifiers {
        let value = evaluate_linear(modifier.base, modifier.per_level, level) as f32;
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
    ConfigError::new("ability report", error.to_string())
}
