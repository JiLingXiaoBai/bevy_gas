use super::data::{
    ActionKind, AttributeRegion, DurationKind, MagnitudeKind, ModifierOperation, SelectionKind,
    SortOrder, TargetScope,
};
use super::{ConfigError, Tables, evaluate_linear};
use crate::{COLD_ATTRIBUTE_SET_SIZE, HOT_ATTRIBUTE_SET_SIZE, MAX_TAG_COUNTS};
use std::collections::BTreeSet;

const MAX_CONFIG_TICKS: i32 = 1_000_000;
const MAX_CONFIG_LEVEL: i32 = 100;

/// Validates references, timing, formulas, task ordering, and GAS-specific constraints.
///
/// `tables` are generated Luban rows. Returns `Ok(())` when they can be compiled
/// against empty registries, or the first contextual semantic error. Existing
/// runtime registry capacity and registration conflicts are checked at compilation.
pub fn validate_tables(tables: &Tables) -> Result<(), ConfigError> {
    validate_names(tables)?;
    validate_effects(tables)?;
    validate_modifiers(tables)?;
    validate_targeting(tables)?;
    validate_actions(tables)?;
    validate_abilities(tables)
}

fn require(
    condition: bool,
    context: impl Into<String>,
    message: impl Into<String>,
) -> Result<(), ConfigError> {
    if condition {
        Ok(())
    } else {
        Err(ConfigError::new(context, message))
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
}

fn validate_names(tables: &Tables) -> Result<(), ConfigError> {
    let mut names = BTreeSet::new();
    let mut expanded_tags = BTreeSet::new();
    for row in tables.tb_tag.iter() {
        let context = format!("Tag[{}].name", row.name);
        require(
            valid_name(&row.name),
            &context,
            "use nonempty dot-separated ASCII name segments",
        )?;
        require(names.insert(&row.name), &context, "duplicate tag name")?;
        let mut name = row.name.as_str();
        loop {
            expanded_tags.insert(name);
            let Some((parent, _)) = name.rsplit_once('.') else {
                break;
            };
            name = parent;
        }
    }
    require(
        expanded_tags.len() <= MAX_TAG_COUNTS,
        "Tag",
        "tag capacity exceeded including implicit parent tags",
    )?;
    let mut names = BTreeSet::new();
    let mut hot = 0;
    let mut cold = 0;
    for row in tables.tb_attribute.iter() {
        let context = format!("Attribute[{}].name", row.name);
        require(
            valid_name(&row.name),
            &context,
            "use a nonempty stable ASCII name",
        )?;
        require(
            names.insert(&row.name),
            &context,
            "duplicate attribute name",
        )?;
        match row.region {
            AttributeRegion::Hot => hot += 1,
            AttributeRegion::Cold => cold += 1,
        }
    }
    require(
        hot <= HOT_ATTRIBUTE_SET_SIZE,
        "Attribute.region",
        "hot attribute capacity exceeded",
    )?;
    require(
        cold <= COLD_ATTRIBUTE_SET_SIZE,
        "Attribute.region",
        "cold attribute capacity exceeded",
    )
}

fn validate_tag_refs(tables: &Tables, tags: &[String], context: &str) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for tag in tags {
        require(
            tables.tb_tag.get(tag).is_some(),
            context,
            format!("unknown tag '{tag}'; referenced parent tags must also be declared"),
        )?;
        require(seen.insert(tag), context, format!("duplicate tag '{tag}'"))?;
    }
    Ok(())
}

fn validate_tag_requirements(
    tables: &Tables,
    required: &[String],
    blocked: &[String],
    context: &str,
) -> Result<(), ConfigError> {
    validate_tag_refs(tables, required, &format!("{context}.required_tags"))?;
    validate_tag_refs(tables, blocked, &format!("{context}.blocked_tags"))?;
    for required in required {
        for blocked in blocked {
            require(
                required != blocked
                    && !required
                        .strip_prefix(blocked)
                        .is_some_and(|suffix| suffix.starts_with('.')),
                context,
                format!("required tag '{required}' implies blocked tag '{blocked}'"),
            )?;
        }
    }
    Ok(())
}

fn validate_effects(tables: &Tables) -> Result<(), ConfigError> {
    let mut ids = BTreeSet::new();
    for row in tables.tb_effect.iter() {
        let context = format!("Effect[{}]", row.id);
        require(
            row.id > 0 && ids.insert(row.id),
            &context,
            "effect ID must be positive and unique",
        )?;
        require(
            !row.name.trim().is_empty(),
            format!("{context}.name"),
            "name must not be blank",
        )?;
        require(
            row.probability.is_finite() && (0.0..=1.0).contains(&row.probability),
            format!("{context}.probability"),
            "probability must be finite and in 0..=1",
        )?;
        match row.duration_kind {
            DurationKind::DurationTicks => require(
                row.duration_ticks
                    .is_some_and(|ticks| (1..=MAX_CONFIG_TICKS).contains(&ticks)),
                format!("{context}.duration_ticks"),
                "duration ticks must be an integer in 1..=1000000",
            )?,
            DurationKind::Instant | DurationKind::Infinite => require(
                row.duration_ticks.is_none(),
                format!("{context}.duration_ticks"),
                "unused duration must be empty",
            )?,
        }
        if let Some(period) = row.period_ticks {
            require(
                row.duration_kind != DurationKind::Instant,
                format!("{context}.period_ticks"),
                "instant effects cannot be periodic",
            )?;
            require(
                (1..=MAX_CONFIG_TICKS).contains(&period),
                format!("{context}.period_ticks"),
                "period ticks must be in 1..=1000000; empty disables periodic execution",
            )?;
        } else {
            require(
                !row.execute_on_applied,
                format!("{context}.execute_on_applied"),
                "execute_on_applied requires a period",
            )?;
        }
        validate_tag_refs(tables, &row.asset_tags, &format!("{context}.asset_tags"))?;
        validate_tag_refs(
            tables,
            &row.granted_tags,
            &format!("{context}.granted_tags"),
        )?;
        if row.duration_kind == DurationKind::Instant {
            require(
                row.granted_tags.is_empty(),
                format!("{context}.granted_tags"),
                "instant effects cannot retain granted tags",
            )?;
        }
    }
    Ok(())
}

fn validate_modifiers(tables: &Tables) -> Result<(), ConfigError> {
    let mut ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for row in tables.tb_modifier.iter() {
        let context = format!("Modifier[{}]", row.id);
        require(
            row.id > 0 && ids.insert(row.id),
            &context,
            "modifier ID must be positive and unique",
        )?;
        require(
            tables.tb_effect.get(&row.effect_id).is_some(),
            format!("{context}.effect_id"),
            "unknown effect",
        )?;
        require(
            tables.tb_attribute.get(&row.attribute).is_some(),
            format!("{context}.attribute"),
            "unknown attribute",
        )?;
        require(
            row.order >= 0 && positions.insert((row.effect_id, row.order)),
            format!("{context}.order"),
            "order must be nonnegative and unique within the effect",
        )?;
        require(
            row.base.is_finite() && row.per_level.is_finite(),
            format!("{context}.magnitude"),
            "formula parameters must be finite",
        )?;
        if row.magnitude_kind == MagnitudeKind::Flat {
            require(
                row.per_level == 0.0,
                format!("{context}.per_level"),
                "Flat magnitude requires unused per_level to be zero",
            )?;
        }
    }
    Ok(())
}

fn validate_targeting(tables: &Tables) -> Result<(), ConfigError> {
    let mut ids = BTreeSet::new();
    for row in tables.tb_targeting.iter() {
        let context = format!("Targeting[{}]", row.id);
        require(
            row.id > 0 && ids.insert(row.id),
            &context,
            "targeting ID must be positive and unique",
        )?;
        require(
            !row.name.trim().is_empty(),
            format!("{context}.name"),
            "name must not be blank",
        )?;
        match row.selection {
            SelectionKind::Sphere | SelectionKind::Cone => require(
                row.radius
                    .is_some_and(|v| v.is_finite() && v >= 0.0 && (v * v).is_finite()),
                format!("{context}.radius"),
                "sphere/cone radius must be finite, nonnegative, and have a finite square",
            )?,
            SelectionKind::SelfTarget | SelectionKind::ExplicitEntity => require(
                row.radius.is_none(),
                format!("{context}.radius"),
                "unused radius must be empty",
            )?,
        }
        if row.selection == SelectionKind::Cone {
            require(
                row.half_angle_radians
                    .is_some_and(|v| v.is_finite() && (0.0..=std::f32::consts::PI).contains(&v)),
                format!("{context}.half_angle_radians"),
                "cone half angle must be finite and in 0..=PI",
            )?;
        } else {
            require(
                row.half_angle_radians.is_none(),
                format!("{context}.half_angle_radians"),
                "unused cone angle must be empty",
            )?;
        }
        if let Some(distance) = row.max_distance {
            require(
                distance.is_finite() && distance >= 0.0 && (distance * distance).is_finite(),
                format!("{context}.max_distance"),
                "distance must be finite, nonnegative, and have a finite square",
            )?;
        }
        if row.selection == SelectionKind::SelfTarget {
            require(
                !row.exclude_source,
                format!("{context}.exclude_source"),
                "self selection cannot exclude its only target",
            )?;
        }
        if matches!(
            row.selection,
            SelectionKind::SelfTarget | SelectionKind::ExplicitEntity
        ) {
            require(
                row.sort == SortOrder::None && row.limit == 1,
                &context,
                "single-entity selection requires sort=None and limit=1",
            )?;
        }
        require(
            row.limit > 0,
            format!("{context}.limit"),
            "target limit must be positive",
        )?;
        validate_tag_requirements(tables, &row.required_tags, &row.blocked_tags, &context)?;
    }
    Ok(())
}

fn validate_actions(tables: &Tables) -> Result<(), ConfigError> {
    let mut ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for row in tables.tb_ability_action.iter() {
        let context = format!("AbilityAction[{}]", row.id);
        require(
            row.id > 0 && ids.insert(row.id),
            &context,
            "action ID must be positive and unique",
        )?;
        require(
            tables.tb_ability.get(&row.ability_id).is_some(),
            format!("{context}.ability_id"),
            "unknown ability",
        )?;
        require(
            (0..=MAX_CONFIG_TICKS).contains(&row.at_tick),
            format!("{context}.at_tick"),
            "at_tick must be in 0..=1000000; zero means activation startup",
        )?;
        require(
            row.order >= 0 && positions.insert((row.ability_id, row.at_tick, row.order)),
            format!("{context}.order"),
            "(ability_id, at_tick, order) must be unique and order nonnegative",
        )?;
        match row.kind {
            ActionKind::ApplyEffect => {
                require(
                    row.effect_id
                        .is_some_and(|id| tables.tb_effect.get(&id).is_some()),
                    format!("{context}.effect_id"),
                    "ApplyEffect requires a valid effect reference",
                )?;
                require(
                    matches!(
                        row.target_scope,
                        TargetScope::Primary | TargetScope::AllCaptured
                    ),
                    format!("{context}.target_scope"),
                    "ApplyEffect requires Primary or AllCaptured",
                )?;
            }
            ActionKind::EndAbility => require(
                row.effect_id.is_none() && row.target_scope == TargetScope::None,
                &context,
                "EndAbility requires empty effect_id and target_scope=None",
            )?,
        }
    }
    Ok(())
}

fn validate_abilities(tables: &Tables) -> Result<(), ConfigError> {
    let mut ids = BTreeSet::new();
    for row in tables.tb_ability.iter() {
        let context = format!("Ability[{}]", row.id);
        require(
            row.id > 0 && ids.insert(row.id),
            &context,
            "ability ID must be positive and unique",
        )?;
        require(
            !row.name.trim().is_empty(),
            format!("{context}.name"),
            "name must not be blank",
        )?;
        require(
            (1..=MAX_CONFIG_LEVEL).contains(&row.max_level),
            format!("{context}.max_level"),
            "max_level must be in 1..=100",
        )?;
        require(
            tables.tb_targeting.get(&row.targeting_id).is_some(),
            format!("{context}.targeting_id"),
            "unknown targeting definition",
        )?;
        validate_tag_refs(tables, &row.asset_tags, &format!("{context}.asset_tags"))?;
        validate_tag_refs(
            tables,
            &row.cancel_ability_tags,
            &format!("{context}.cancel_ability_tags"),
        )?;
        validate_tag_refs(
            tables,
            &row.block_ability_tags,
            &format!("{context}.block_ability_tags"),
        )?;
        validate_tag_requirements(tables, &row.required_tags, &row.blocked_tags, &context)?;
        let mut effects: BTreeSet<i32> = row.activation_effect_ids.iter().copied().collect();
        effects.extend(row.cost_effect_id);
        effects.extend(row.cooldown_effect_id);
        let mut actions: Vec<_> = tables
            .tb_ability_action
            .iter()
            .filter(|action| action.ability_id == row.id)
            .collect();
        actions.sort_by_key(|action| (action.at_tick, action.order));
        let ends = actions
            .iter()
            .filter(|action| action.kind == ActionKind::EndAbility)
            .count();
        if row.end_on_activation {
            require(
                actions.iter().all(|action| action.at_tick == 0),
                format!("{context}.end_on_activation"),
                "waiting actions cannot run after end_on_activation",
            )?;
            require(
                ends == 0,
                &context,
                "end_on_activation makes explicit EndAbility actions redundant",
            )?;
        } else {
            require(
                ends == 1
                    && actions
                        .last()
                        .is_some_and(|action| action.kind == ActionKind::EndAbility),
                &context,
                "exactly one EndAbility must be the final ordered action",
            )?;
        }
        for action in actions {
            effects.extend(action.effect_id);
        }
        for effect_id in effects {
            require(
                tables.tb_effect.get(&effect_id).is_some(),
                &context,
                format!("unknown effect {effect_id}"),
            )?;
            for modifier in tables
                .tb_modifier
                .iter()
                .filter(|modifier| modifier.effect_id == effect_id)
            {
                let maximum =
                    evaluate_linear(modifier.base, modifier.per_level, row.max_level as u32);
                require(
                    maximum.is_finite() && maximum.abs() <= f32::MAX as f64,
                    format!("{context}.Effect[{effect_id}].Modifier[{}]", modifier.id),
                    "formula exceeds finite f32 range within supported levels",
                )?;
            }
        }
        if let Some(cost_id) = row.cost_effect_id {
            validate_cost(tables, cost_id, row.max_level, &context)?;
        }
        if let Some(cooldown_id) = row.cooldown_effect_id {
            validate_cooldown(tables, cooldown_id, &context)?;
        }
    }
    Ok(())
}

fn validate_cost(
    tables: &Tables,
    id: i32,
    max_level: i32,
    ability_context: &str,
) -> Result<(), ConfigError> {
    let context = format!("{ability_context}.cost_effect_id({id})");
    let effect = tables
        .tb_effect
        .get(&id)
        .ok_or_else(|| ConfigError::new(&context, "unknown effect"))?;
    require(
        effect.duration_kind == DurationKind::Instant
            && effect.period_ticks.is_none()
            && effect.probability == 1.0,
        &context,
        "cost must be instant, nonperiodic, and probability=1",
    )?;
    let mut attributes = BTreeSet::new();
    let mut count = 0;
    for modifier in tables
        .tb_modifier
        .iter()
        .filter(|modifier| modifier.effect_id == id)
    {
        count += 1;
        require(
            modifier.operation == ModifierOperation::Add,
            &context,
            "cost modifiers must use Add",
        )?;
        require(
            attributes.insert(&modifier.attribute),
            &context,
            "cost cannot contain duplicate attributes",
        )?;
        require(
            modifier.base < 0.0
                && evaluate_linear(modifier.base, modifier.per_level, max_level as u32) < 0.0,
            &context,
            "cost magnitude must remain negative at all supported levels",
        )?;
    }
    require(
        count > 0,
        &context,
        "cost requires at least one attribute modifier",
    )
}

fn validate_cooldown(tables: &Tables, id: i32, ability_context: &str) -> Result<(), ConfigError> {
    let context = format!("{ability_context}.cooldown_effect_id({id})");
    let effect = tables
        .tb_effect
        .get(&id)
        .ok_or_else(|| ConfigError::new(&context, "unknown effect"))?;
    require(
        effect.duration_kind == DurationKind::DurationTicks
            && effect.period_ticks.is_none()
            && effect.probability == 1.0
            && !effect.granted_tags.is_empty(),
        &context,
        "cooldown requires positive duration, granted tags, probability=1, and no period",
    )?;
    require(
        !tables
            .tb_modifier
            .iter()
            .any(|modifier| modifier.effect_id == id),
        &context,
        "cooldown effects must not modify attributes",
    )
}
