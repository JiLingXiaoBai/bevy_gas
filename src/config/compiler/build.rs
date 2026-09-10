use super::{
    AbilityId, CompiledAbility, ConfigError, EffectId, GameplayCatalog, Tables, data,
    evaluate_linear, validate_tables,
};
use crate::{
    AbilityTags, AbilityTaskDef, AbilityTaskOnFinishedDef, AttributeId, AttributeIdManager,
    AttributeRegion, EffectDurationTicks, EffectPeriodTicks, EffectTags, GameplayAbility,
    GameplayEffect, GameplayTag, GameplayTagBits, GameplayTagManager, Modifier,
    ModifierEvaluationContext, ModifierMagnitude, ModifierMagnitudeCalculation, ModifierOperation,
    StackingPolicy, TagRequirements, TargetingDefinition, TargetingOperation, TargetingSortOrder,
    UniqueNamePool, add_bit_with_tag,
};
use bevy::prelude::World;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Validates `tables`, registers their stable names, and compiles shared definitions.
///
/// `world` must contain `UniqueNamePool`, `GameplayTagManager`, and
/// `AttributeIdManager`, normally installed by `GameplayAbilitySystemPlugin`.
/// Returns an unpublished catalog, or a contextual configuration/registration error.
/// All table validation happens before mutation. Registration is append-only and
/// may leave successfully registered names after a later registration failure.
/// Call this during startup; replacing catalogs during combat is unsupported.
pub fn compile_catalog(tables: &Tables, world: &mut World) -> Result<GameplayCatalog, ConfigError> {
    validate_tables(tables)?;
    if !world.contains_resource::<GameplayTagManager>()
        || !world.contains_resource::<AttributeIdManager>()
    {
        return Err(ConfigError::new(
            "runtime registries",
            "install GameplayAbilitySystemPlugin before compiling configuration",
        ));
    }
    let mut names = world.remove_resource::<UniqueNamePool>().ok_or_else(|| {
        ConfigError::new(
            "UniqueNamePool",
            "install GameplayAbilitySystemPlugin before compiling configuration",
        )
    })?;
    let result = compile_with_names(tables, world, &mut names);
    world.insert_resource(names);
    result
}

fn compile_with_names(
    tables: &Tables,
    world: &mut World,
    names: &mut UniqueNamePool,
) -> Result<GameplayCatalog, ConfigError> {
    let tags = {
        let mut manager = world
            .get_resource_mut::<GameplayTagManager>()
            .ok_or_else(|| {
                ConfigError::new("GameplayTagManager", "required registry is missing")
            })?;
        register_tags(tables, names, &mut manager)?
    };
    let attributes = {
        let mut manager = world
            .get_resource_mut::<AttributeIdManager>()
            .ok_or_else(|| {
                ConfigError::new("AttributeIdManager", "required registry is missing")
            })?;
        register_attributes(tables, names, &mut manager)?
    };
    let effects = compile_effects(tables, &tags, &attributes)?;
    let mut targeting = BTreeMap::new();
    for row in tables.tb_targeting.iter() {
        targeting.insert(row.id, Arc::new(compile_targeting(row, &tags)?));
    }
    let mut abilities = BTreeMap::new();
    for row in tables.tb_ability.iter() {
        let ability_tags = AbilityTags::default()
            .with_ability_asset_tags(resolve_tags(&row.asset_tags, &tags)?)
            .with_activation_required_tags(resolve_tags(&row.required_tags, &tags)?)
            .with_activation_blocked_tags(resolve_tags(&row.blocked_tags, &tags)?)
            .with_cancel_abilities_with_tags(resolve_tags(&row.cancel_ability_tags, &tags)?)
            .with_block_abilities_with_tags(resolve_tags(&row.block_ability_tags, &tags)?);
        let activation_effects = row
            .activation_effect_ids
            .iter()
            .map(|id| resolve_effect(*id, &effects))
            .collect::<Result<Vec<_>, _>>()?;
        let mut definition = GameplayAbility::default()
            .with_tags(ability_tags)
            .with_startup_tasks(compile_tasks(tables, row.id, &effects)?)
            .with_activation_effects(activation_effects)
            .with_end_on_activation(row.end_on_activation)
            .with_allow_multiple_instances(row.allow_multiple_instances);
        if let Some(id) = row.cost_effect_id {
            definition = definition.with_cost(resolve_effect(id, &effects)?);
        }
        if let Some(id) = row.cooldown_effect_id {
            definition = definition.with_cooldown(resolve_effect(id, &effects)?);
        }
        let target = targeting.get(&row.targeting_id).ok_or_else(|| {
            ConfigError::new(
                format!("Ability[{}].targeting_id", row.id),
                "unresolved targeting",
            )
        })?;
        let max_level = u32::try_from(row.max_level).map_err(|error| {
            ConfigError::new(format!("Ability[{}].max_level", row.id), error.to_string())
        })?;
        abilities.insert(
            AbilityId(row.id),
            CompiledAbility {
                id: AbilityId(row.id),
                name: row.name.clone(),
                max_level,
                definition: Arc::new(definition),
                targeting: Arc::clone(target),
            },
        );
    }
    Ok(GameplayCatalog {
        abilities,
        effects,
        attributes,
        tags,
    })
}

fn register_tags(
    tables: &Tables,
    names: &mut UniqueNamePool,
    manager: &mut GameplayTagManager,
) -> Result<BTreeMap<String, GameplayTag>, ConfigError> {
    let mut ordered = BTreeSet::new();
    for row in tables.tb_tag.iter() {
        let mut name = row.name.as_str();
        loop {
            ordered.insert(name);
            let Some((parent, _)) = name.rsplit_once('.') else {
                break;
            };
            name = parent;
        }
    }
    let mut tags: BTreeMap<String, GameplayTag> = BTreeMap::new();
    for name in ordered {
        let context = format!("Tag[{name}]");
        let unique = names
            .new_name(name)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let parent = if let Some((parent, _)) = name.rsplit_once('.') {
            Some(
                *tags
                    .get(parent)
                    .ok_or_else(|| ConfigError::new(&context, "parent tag was not registered"))?,
            )
        } else {
            None
        };
        let mut expected_bits = match parent {
            Some(parent) => *manager
                .get_inherited_bits(&parent)
                .map_err(|error| ConfigError::new(&context, error.to_string()))?,
            None => GameplayTagBits::default(),
        };
        let tag = manager
            .register_tag_internal(unique, parent.map(|tag| tag.get_bit_index_u16()))
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        add_bit_with_tag(&mut expected_bits, &tag)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let actual_bits = manager
            .get_inherited_bits(&tag)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        if *actual_bits != expected_bits {
            return Err(ConfigError::new(
                &context,
                "existing tag inheritance conflicts with the configured hierarchy",
            ));
        }
        tags.insert(name.to_owned(), tag);
    }
    Ok(tags)
}

fn register_attributes(
    tables: &Tables,
    names: &mut UniqueNamePool,
    manager: &mut AttributeIdManager,
) -> Result<BTreeMap<String, AttributeId>, ConfigError> {
    let mut rows: Vec<_> = tables.tb_attribute.iter().collect();
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    let mut attributes = BTreeMap::new();
    for row in rows {
        let context = format!("Attribute[{}]", row.name);
        let unique = names
            .new_name(&row.name)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let region = match row.region {
            data::AttributeRegion::Hot => AttributeRegion::Hot,
            data::AttributeRegion::Cold => AttributeRegion::Cold,
        };
        let id = manager
            .register_id_internal(unique, region)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        attributes.insert(row.name.clone(), id);
    }
    Ok(attributes)
}

fn resolve_tags(
    names: &[String],
    tags: &BTreeMap<String, GameplayTag>,
) -> Result<Vec<GameplayTag>, ConfigError> {
    names
        .iter()
        .map(|name| {
            tags.get(name).copied().ok_or_else(|| {
                ConfigError::new(format!("Tag[{name}]"), "unresolved registered tag")
            })
        })
        .collect()
}

fn resolve_effect(
    id: i32,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
) -> Result<Arc<GameplayEffect>, ConfigError> {
    effects
        .get(&EffectId(id))
        .map(Arc::clone)
        .ok_or_else(|| ConfigError::new(format!("Effect[{id}]"), "unresolved effect reference"))
}

struct LinearLevelMagnitude {
    base: f32,
    per_level: f32,
}

impl ModifierMagnitudeCalculation for LinearLevelMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        let value = evaluate_linear(self.base, self.per_level, context.level());
        if value.is_finite() && value.abs() <= f64::from(f32::MAX) {
            value as f32
        } else {
            0.0
        }
    }
}

fn compile_effects(
    tables: &Tables,
    tags: &BTreeMap<String, GameplayTag>,
    attributes: &BTreeMap<String, AttributeId>,
) -> Result<BTreeMap<EffectId, Arc<GameplayEffect>>, ConfigError> {
    let mut effects = BTreeMap::new();
    for row in tables.tb_effect.iter() {
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
            data::DurationKind::DurationTicks => EffectDurationTicks::DurationTicks(
                ModifierMagnitude::Flat(row.duration_ticks.ok_or_else(|| {
                    ConfigError::new(
                        format!("Effect[{}].duration_ticks", row.id),
                        "missing duration",
                    )
                })? as f32),
            ),
        };
        let period = row.period_ticks.map(|ticks| {
            EffectPeriodTicks::new(
                ModifierMagnitude::Flat(ticks as f32),
                row.execute_on_applied,
            )
        });
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

fn compile_targeting(
    row: &data::Targeting,
    tags: &BTreeMap<String, GameplayTag>,
) -> Result<TargetingDefinition, ConfigError> {
    let context = format!("Targeting[{}]", row.id);
    let selection = match row.selection {
        data::SelectionKind::SelfTarget => TargetingOperation::SelectSelf,
        data::SelectionKind::ExplicitEntity => TargetingOperation::SelectExplicitEntity,
        data::SelectionKind::Sphere => TargetingOperation::SelectSphere {
            radius: row
                .radius
                .ok_or_else(|| ConfigError::new(&context, "missing radius"))?,
        },
        data::SelectionKind::Cone => TargetingOperation::SelectCone {
            radius: row
                .radius
                .ok_or_else(|| ConfigError::new(&context, "missing radius"))?,
            half_angle_radians: row
                .half_angle_radians
                .ok_or_else(|| ConfigError::new(&context, "missing cone angle"))?,
        },
    };
    let mut operations = vec![selection];
    if row.exclude_source {
        operations.push(TargetingOperation::FilterSource);
    }
    if !row.required_tags.is_empty() || !row.blocked_tags.is_empty() {
        let requirements = TagRequirements::new(
            resolve_tags(&row.required_tags, tags)?,
            resolve_tags(&row.blocked_tags, tags)?,
        )
        .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        operations.push(TargetingOperation::FilterTags { requirements });
    }
    if row.require_attributes {
        operations.push(TargetingOperation::RequireAttributeSet);
    }
    if let Some(max_distance) = row.max_distance {
        operations.push(TargetingOperation::FilterDistance { max_distance });
    }
    match row.sort {
        data::SortOrder::None => {}
        data::SortOrder::Nearest => operations.push(TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        }),
        data::SortOrder::Farthest => operations.push(TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Descending,
        }),
    }
    operations.push(TargetingOperation::Limit {
        count: usize::try_from(row.limit)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?,
    });
    TargetingDefinition::new(operations)
        .map_err(|error| ConfigError::new(context, error.to_string()))
}

fn compile_tasks(
    tables: &Tables,
    ability_id: i32,
    effects: &BTreeMap<EffectId, Arc<GameplayEffect>>,
) -> Result<Vec<AbilityTaskDef>, ConfigError> {
    let mut rows: Vec<_> = tables
        .tb_ability_action
        .iter()
        .filter(|action| action.ability_id == ability_id)
        .collect();
    rows.sort_by_key(|action| (action.at_tick, action.order));
    let mut groups: BTreeMap<u32, Vec<AbilityTaskOnFinishedDef>> = BTreeMap::new();
    for row in rows {
        let context = format!("AbilityAction[{}]", row.id);
        let action = match row.kind {
            data::ActionKind::EndAbility => AbilityTaskOnFinishedDef::EndAbility,
            data::ActionKind::ApplyEffect => {
                let effect = resolve_effect(
                    row.effect_id
                        .ok_or_else(|| ConfigError::new(&context, "missing effect reference"))?,
                    effects,
                )?;
                match row.target_scope {
                    data::TargetScope::Primary => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect }
                    }
                    data::TargetScope::AllCaptured => {
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect }
                    }
                    data::TargetScope::None => {
                        return Err(ConfigError::new(
                            &context,
                            "ApplyEffect needs a target scope",
                        ));
                    }
                }
            }
        };
        let tick = u32::try_from(row.at_tick)
            .map_err(|error| ConfigError::new(context, error.to_string()))?;
        groups.entry(tick).or_default().push(action);
    }
    Ok(groups
        .into_iter()
        .map(|(tick, actions)| {
            let batch = AbilityTaskOnFinishedDef::Batch { actions };
            if tick == 0 {
                AbilityTaskDef::instant(batch)
            } else {
                AbilityTaskDef::wait_ticks(tick, batch)
            }
        })
        .collect())
}
