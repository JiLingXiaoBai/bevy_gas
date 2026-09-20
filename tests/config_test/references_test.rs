use super::runtime_test::{app, tables as base_tables};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gas::config::generated::gas::{
    AbilityAdditionalCost, Attribute, AttributeRegion, DurationKind, Effect, MagnitudeKind,
    Modifier, ModifierOperation, TbAbility, TbAbilityAdditionalCost, TbAbilityTask, TbAttribute,
    TbEffect, TbModifier,
};
use bevy_gas::config::generated::{ByteBuf, Tables};
use bevy_gas::config::{ConfigErrorKind, ConfigLocation, EffectId, compile_catalog};
use bevy_gas::{
    AttributeIdManager, AttributeSet, EffectContext, EffectPayload, GameplayTagContainer,
};
use std::sync::Arc;

fn effect(id: i32, modifier_ids: Vec<i32>) -> Effect {
    Effect {
        id,
        name: format!("Effect {id}"),
        duration_kind: DurationKind::Instant,
        duration_ticks: None,
        period_ticks: None,
        execute_on_applied: false,
        probability: 1.0,
        asset_tags: vec![],
        granted_tags: vec![],
        modifier_ids,
    }
}

fn modifier(id: i32, base: f32) -> Modifier {
    Modifier {
        id,
        attribute: "Health".to_owned(),
        operation: ModifierOperation::Add,
        magnitude_kind: MagnitudeKind::Flat,
        base,
        per_level: 0.0,
    }
}

#[test]
fn missing_and_duplicate_child_ids_identify_the_parent_reference_field() {
    for field in ["task_ids", "additional_cost_ids", "modifier_ids"] {
        for (ids, kind) in [
            (vec![99], ConfigErrorKind::Reference),
            (vec![1, 1], ConfigErrorKind::InvalidValue),
        ] {
            let mut tables = base_tables(None);
            let mut ability = tables.tb_ability.get(&1).unwrap().as_ref().clone();
            let table = match field {
                "task_ids" => {
                    ability.task_ids = ids;
                    "Ability"
                }
                "additional_cost_ids" => {
                    ability.additional_cost_ids = ids;
                    tables.tb_ability_additional_cost = Arc::new(
                        TbAbilityAdditionalCost::from_rows(vec![AbilityAdditionalCost {
                            id: 1,
                            resource: "Inventory.Bomb".to_owned(),
                            amount: 1,
                        }])
                        .unwrap(),
                    );
                    "Ability"
                }
                "modifier_ids" => {
                    tables.tb_effect = Arc::new(TbEffect::from_rows(vec![effect(1, ids)]).unwrap());
                    tables.tb_modifier =
                        Arc::new(TbModifier::from_rows(vec![modifier(1, 1.0)]).unwrap());
                    "Effect"
                }
                _ => unreachable!(),
            };
            tables.tb_ability = Arc::new(TbAbility::from_rows(vec![ability]).unwrap());
            let error = compile_catalog(&tables, app().world_mut()).err().unwrap();
            assert_eq!(error.kind(), kind, "{field}");
            assert_eq!(
                error.location(),
                &ConfigLocation::Table {
                    table,
                    row: Some("1".to_owned()),
                    field: Some(field.to_owned()),
                }
            );
        }
    }
}

#[test]
fn effects_share_modifier_definitions_with_independent_list_order() {
    for reverse in [false, true] {
        let mut tables = base_tables(None);
        tables.tb_attribute = Arc::new(
            TbAttribute::from_rows(vec![Attribute {
                name: "Health".to_owned(),
                region: AttributeRegion::Hot,
                description: String::new(),
            }])
            .unwrap(),
        );
        tables.tb_effect = Arc::new(
            TbEffect::from_rows(vec![
                effect(1, vec![2, 1]),
                effect(2, vec![1, 2]),
                effect(3, vec![]),
            ])
            .unwrap(),
        );
        let mut modifiers = vec![modifier(1, 10.0), modifier(2, 20.0)];
        if reverse {
            modifiers.reverse();
        }
        tables.tb_modifier = Arc::new(TbModifier::from_rows(modifiers).unwrap());
        let mut app = app();
        let catalog = compile_catalog(&tables, app.world_mut()).unwrap();
        for (id, expected) in [(1, vec![20.0, 10.0]), (2, vec![10.0, 20.0]), (3, vec![])] {
            let definition = Arc::clone(catalog.effect(EffectId(id)).unwrap());
            let actual = app
                .world_mut()
                .run_system_once(
                    move |manager: Res<AttributeIdManager>,
                          attributes: Query<&'static AttributeSet>,
                          tags: Query<&'static GameplayTagContainer>| {
                        let payload = EffectPayload::new(Entity::PLACEHOLDER, None, 1);
                        let context = EffectContext {
                            target: None,
                            payload: &payload,
                            attribute_id_manager: &manager,
                            attr_set_query: &attributes,
                            tag_container_query: &tags,
                        };
                        definition
                            .make_spec(&context)
                            .get_modifier_specs()
                            .iter()
                            .map(|modifier| modifier.get_value())
                            .collect::<Vec<_>>()
                    },
                )
                .unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn unused_child_definitions_are_only_checked_by_authoring_validation() {
    for (table, invalid) in ["AbilityTask", "AbilityAdditionalCost", "Modifier"]
        .into_iter()
        .flat_map(|table| [false, true].map(|invalid| (table, invalid)))
    {
        let mut tables = base_tables(None);
        match table {
            "AbilityTask" => {
                let original = tables.tb_ability_task.get(&1).unwrap().as_ref().clone();
                let mut unused = original.clone();
                unused.id = 99;
                unused.at_tick = if invalid { -1 } else { 0 };
                tables.tb_ability_task =
                    Arc::new(TbAbilityTask::from_rows(vec![original, unused]).unwrap());
            }
            "AbilityAdditionalCost" => {
                tables.tb_ability_additional_cost = Arc::new(
                    TbAbilityAdditionalCost::from_rows(vec![AbilityAdditionalCost {
                        id: 99,
                        resource: "Inventory.Unused".to_owned(),
                        amount: if invalid { 0 } else { 1 },
                    }])
                    .unwrap(),
                );
            }
            "Modifier" => {
                tables.tb_attribute = Arc::new(
                    TbAttribute::from_rows(vec![Attribute {
                        name: "Health".to_owned(),
                        region: AttributeRegion::Hot,
                        description: String::new(),
                    }])
                    .unwrap(),
                );
                tables.tb_modifier = Arc::new(
                    TbModifier::from_rows(vec![modifier(99, if invalid { f32::NAN } else { 1.0 })])
                        .unwrap(),
                );
            }
            _ => unreachable!(),
        }
        let result = compile_catalog(&tables, app().world_mut());
        if invalid && cfg!(feature = "config-validation") {
            let error = result.err().unwrap();
            assert!(
                matches!(error.location(), ConfigLocation::Table { table: actual, row: Some(row), .. } if *actual == table && row == "99")
            );
        } else {
            let catalog = result.unwrap();
            assert_eq!(catalog.abilities().count(), 1);
            assert_eq!(catalog.effect_count(), 0);
        }
    }
}

#[test]
fn binary_rows_decode_parent_lists_and_independent_child_definitions() {
    // All integers fit in one byte; f32 values use little-endian IEEE 754 bytes.
    let ability = vec![1, 7, 1, b'A', 2, 0, 1, 6, 0, 0, 0, 0, 0, 0, 1, 0, 2, 9, 8];
    let task = vec![1, 9, 3, 1, 0, 0];
    let mut effect = vec![1, 4, 1, b'E', 0, 0, 0, 0];
    effect.extend_from_slice(&1.0_f32.to_le_bytes());
    effect.extend_from_slice(&[0, 0, 2, 3, 2]);
    let mut modifier = vec![1, 3, 6];
    modifier.extend_from_slice(b"Health");
    modifier.extend_from_slice(&[0, 0]);
    modifier.extend_from_slice(&12.0_f32.to_le_bytes());
    modifier.extend_from_slice(&0.0_f32.to_le_bytes());
    let tables = Tables::new(|name| {
        Ok(ByteBuf::new(match name {
            "gas_tbability" => ability.clone(),
            "gas_tbabilitytask" => task.clone(),
            "gas_tbeffect" => effect.clone(),
            "gas_tbmodifier" => modifier.clone(),
            _ => vec![0],
        }))
    })
    .unwrap();
    let ability = tables.tb_ability.get(&7).unwrap();
    assert_eq!(ability.task_ids, [9, 8]);
    assert_eq!(ability.additional_cost_ids, [6]);
    assert_eq!(tables.tb_ability_task.get(&9).unwrap().at_tick, 3);
    assert_eq!(tables.tb_effect.get(&4).unwrap().modifier_ids, [3, 2]);
    assert_eq!(tables.tb_modifier.get(&3).unwrap().base, 12.0);
}
