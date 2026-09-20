use super::runtime_test::{app, tables as base_tables};
use bevy::ecs::system::{SystemParam, SystemParamItem, SystemState};
use bevy::prelude::*;
use bevy_gas::config::generated::gas::{
    AbilityAdditionalCost, TbAbility, TbAbilityAdditionalCost, TbTargeting,
};
use bevy_gas::config::generated::{ByteBuf, Tables};
use bevy_gas::config::{
    AbilityId, ConfigErrorKind, ConfigLocation, ConfiguredAbilities, GameplayCatalog,
    compile_catalog, grant_ability,
};
#[cfg(feature = "config-validation")]
use bevy_gas::config::{describe_ability, describe_ability_at_level, validate_tables};
use bevy_gas::{
    AbilityActivationCheckError, AbilityActivationCheckParams, AbilityActivationContext,
    AbilityActivationError, AbilityCommitError, AdditionalCost, AdditionalCostContext,
    AdditionalCostError, AdditionalCostProvider, GameplayAbilitySystemBundle,
    GameplayAbilitySystemPlugin, GameplayExecutionError, GameplayExecutionOutcome,
    GameplayExecutionQueue, GameplayExecutionResult, UniqueName, UniqueNamePool,
    can_activate_ability,
};
use std::sync::Arc;

fn location(row: &str, field: &str) -> ConfigLocation {
    ConfigLocation::Table {
        table: "AbilityAdditionalCost",
        row: Some(row.to_owned()),
        field: Some(field.to_owned()),
    }
}

fn cost(id: i32, resource: &str, amount: i64) -> AbilityAdditionalCost {
    AbilityAdditionalCost {
        id,
        resource: resource.to_owned(),
        amount,
    }
}

fn tables(rows: Vec<AbilityAdditionalCost>) -> Tables {
    let mut ids: Vec<_> = rows.iter().map(|row| row.id).collect();
    ids.sort_unstable();
    tables_with_ids(rows, ids)
}

fn tables_with_ids(rows: Vec<AbilityAdditionalCost>, ids: Vec<i32>) -> Tables {
    let mut tables = base_tables(None);
    let mut ability = tables.tb_ability.get(&1).unwrap().as_ref().clone();
    ability.additional_cost_ids = ids;
    tables.tb_ability = Arc::new(TbAbility::from_rows(vec![ability]).unwrap());
    tables.tb_ability_additional_cost = Arc::new(TbAbilityAdditionalCost::from_rows(rows).unwrap());
    tables
}

#[test]
fn generated_cost_table_decodes_long_amount_and_compiles_the_requirement() {
    // One row: id 7, Inventory.Bomb, and the largest u32 amount.
    let mut bytes = vec![1, 7, 14];
    bytes.extend_from_slice(b"Inventory.Bomb");
    bytes.extend_from_slice(&[0xf0, 0xff, 0xff, 0xff, 0xff]);
    let decoded = Tables::new(|name| {
        Ok(ByteBuf::new(if name == "gas_tbabilityadditionalcost" {
            bytes.clone()
        } else {
            vec![0]
        }))
    })
    .unwrap();
    let row = decoded.tb_ability_additional_cost.get(&7).unwrap();
    assert_eq!(row.resource, "Inventory.Bomb");
    assert_eq!(row.amount, i64::from(u32::MAX));

    let mut tables = tables_with_ids(vec![], vec![7]);
    tables.tb_ability_additional_cost = decoded.tb_ability_additional_cost;
    let mut app = app();
    let catalog = compile_catalog(&tables, app.world_mut()).unwrap();
    let requirements = catalog
        .ability(AbilityId(1))
        .unwrap()
        .definition()
        .get_additional_costs();
    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].amount(), u32::MAX);
    assert_eq!(
        app.world()
            .resource::<UniqueNamePool>()
            .get_display_str(&requirements[0].resource()),
        "Inventory.Bomb"
    );

    for malformed in [bytes[..bytes.len() - 1].to_vec(), [bytes, vec![0]].concat()] {
        assert!(
            Tables::new(|name| {
                Ok(ByteBuf::new(if name == "gas_tbabilityadditionalcost" {
                    malformed.clone()
                } else {
                    vec![0]
                }))
            })
            .is_err()
        );
    }
}

#[test]
fn cost_order_and_name_registration_are_independent_of_input_row_order() {
    let rows = vec![
        cost(1, "Inventory.Ammo", 1),
        cost(2, "Inventory.ZBomb", 2),
        cost(3, "Inventory.ZBomb", 3),
    ];
    let mut compiled = Vec::new();
    for reverse in [false, true] {
        let mut app = app();
        let mut expected_names = app.world().resource::<UniqueNamePool>().clone();
        let ammo = expected_names.new_name("Inventory.Ammo").unwrap();
        let bomb = expected_names.new_name("Inventory.ZBomb").unwrap();
        let mut rows = rows.clone();
        if reverse {
            rows.reverse();
        }
        let catalog =
            compile_catalog(&tables_with_ids(rows, vec![3, 2, 1]), app.world_mut()).unwrap();
        let requirements = catalog
            .ability(AbilityId(1))
            .unwrap()
            .definition()
            .get_additional_costs();
        assert_eq!(
            requirements,
            [
                AdditionalCost::new(bomb, 3).unwrap(),
                AdditionalCost::new(bomb, 2).unwrap(),
                AdditionalCost::new(ammo, 1).unwrap(),
            ]
        );
        compiled.push(requirements.to_vec());
    }
    assert_eq!(compiled[0], compiled[1]);
}

#[test]
fn malformed_costs_are_rejected_with_field_locations_in_every_feature_mode() {
    let mut cases = Vec::new();
    for amount in [i64::MIN, -1, 0, i64::from(u32::MAX) + 1, i64::MAX] {
        cases.push((vec![cost(1, "Inventory.Bomb", amount)], "1", "amount"));
    }
    for resource in ["", " \t\n"] {
        cases.push((vec![cost(1, resource, 1)], "1", "resource"));
    }
    cases.push((
        vec![
            cost(1, "Inventory.Bomb", i64::from(u32::MAX)),
            cost(2, "Inventory.Bomb", 1),
        ],
        "2",
        "amount",
    ));
    for (rows, row, field) in cases {
        let error = compile_catalog(&tables(rows), app().world_mut())
            .err()
            .unwrap();
        assert_eq!(error.kind(), ConfigErrorKind::InvalidValue);
        assert_eq!(error.location(), &location(row, field));
    }
}

#[test]
fn failed_compilation_does_not_publish_resource_names_or_replace_the_catalog() {
    let mut app = app();
    let original = compile_catalog(&base_tables(None), app.world_mut()).unwrap();
    let original_ability = Arc::clone(original.ability(AbilityId(1)).unwrap().definition());
    app.world_mut().insert_resource(original);
    let names_before = app.world().resource::<UniqueNamePool>().clone();
    let mut invalid = tables(vec![cost(1, "Inventory.Bomb", 1)]);
    invalid.tb_targeting = Arc::new(TbTargeting::from_rows(vec![]).unwrap());
    assert!(compile_catalog(&invalid, app.world_mut()).is_err());

    let mut expected_names = names_before.clone();
    let mut actual_names = app.world().resource::<UniqueNamePool>().clone();
    assert_eq!(
        actual_names.new_name("AfterFailure").unwrap(),
        expected_names.new_name("AfterFailure").unwrap()
    );
    assert!(Arc::ptr_eq(
        app.world()
            .resource::<GameplayCatalog>()
            .ability(AbilityId(1))
            .unwrap()
            .definition(),
        &original_ability
    ));

    let repaired =
        compile_catalog(&tables(vec![cost(1, "Inventory.Bomb", 1)]), app.world_mut()).unwrap();
    let mut expected_names = names_before;
    let expected_bomb = expected_names.new_name("Inventory.Bomb").unwrap();
    assert_eq!(
        repaired
            .ability(AbilityId(1))
            .unwrap()
            .definition()
            .get_additional_costs(),
        [AdditionalCost::new(expected_bomb, 1).unwrap()]
    );
}

#[test]
fn abilities_without_cost_rows_keep_an_empty_requirement_list() {
    let catalog = compile_catalog(&base_tables(None), app().world_mut()).unwrap();
    assert!(
        catalog
            .ability(AbilityId(1))
            .unwrap()
            .definition()
            .get_additional_costs()
            .is_empty()
    );
}

#[cfg(feature = "config-validation")]
#[test]
fn authoring_validation_rejects_malformed_resource_names() {
    for resource in [
        "Inventory..Bomb",
        ".Inventory",
        "Inventory.",
        "Inventory Bomb",
        "背包.炸弹",
    ] {
        let error = validate_tables(&tables(vec![cost(1, resource, 1)])).unwrap_err();
        assert_eq!(error.kind(), ConfigErrorKind::Validation);
        assert_eq!(error.location(), &location("1", "resource"));
    }
}

#[cfg(feature = "config-validation")]
#[test]
fn inspection_displays_ordered_costs_with_constant_amounts_at_each_level() {
    let rows = vec![cost(1, "Inventory.Bomb", 2), cost(2, "Inventory.Ammo", 3)];
    let report =
        describe_ability(&tables_with_ids(rows.clone(), vec![2, 1]), AbilityId(1)).unwrap();
    let mut reversed = rows.clone();
    reversed.reverse();
    assert_eq!(
        report,
        describe_ability(&tables_with_ids(reversed, vec![2, 1]), AbilityId(1)).unwrap()
    );
    let cost_lines: Vec<_> = report
        .lines()
        .filter(|line| line.starts_with("Additional cost"))
        .collect();
    assert_eq!(
        cost_lines,
        [
            "Additional cost / order 0: resource=Inventory.Ammo, amount=3",
            "Additional cost / order 1: resource=Inventory.Bomb, amount=2",
        ]
    );
    let level_two =
        describe_ability_at_level(&tables_with_ids(rows, vec![2, 1]), AbilityId(1), 2).unwrap();
    assert_eq!(
        cost_lines,
        level_two
            .lines()
            .filter(|line| line.starts_with("Additional cost"))
            .collect::<Vec<_>>()
    );
}

#[derive(Component)]
struct Inventory {
    resource: UniqueName,
    bombs: u32,
}

impl Inventory {
    fn required(&self, costs: &[AdditionalCost]) -> Result<u32, AdditionalCostError> {
        let mut required = 0_u32;
        for cost in costs {
            if cost.resource() != self.resource {
                return Err(AdditionalCostError::UnsupportedResource {
                    resource: cost.resource(),
                });
            }
            required = required.checked_add(cost.amount()).unwrap();
        }
        if required > self.bombs {
            return Err(AdditionalCostError::InsufficientResource {
                resource: self.resource,
                required,
                available: self.bombs,
            });
        }
        Ok(required)
    }
}

#[derive(SystemParam)]
struct InventoryProvider<'w, 's> {
    inventories: Query<'w, 's, &'static mut Inventory>,
}

impl AdditionalCostProvider for InventoryProvider<'static, 'static> {
    type ReadOnly = Query<'static, 'static, &'static Inventory>;
    type Receipt = u32;

    fn check(
        params: &SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        params
            .inventories
            .get(context.source)
            .unwrap()
            .required(costs)
            .map(|_| ())
    }

    fn check_readonly(
        params: &SystemParamItem<'_, '_, Self::ReadOnly>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<(), AdditionalCostError> {
        params
            .get(context.source)
            .unwrap()
            .required(costs)
            .map(|_| ())
    }

    fn prepare(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        costs: &[AdditionalCost],
    ) -> Result<Self::Receipt, AdditionalCostError> {
        let mut inventory = params.inventories.get_mut(context.source).unwrap();
        let required = inventory.required(costs)?;
        inventory.bombs -= required;
        Ok(required)
    }

    fn rollback(
        params: &mut SystemParamItem<'_, '_, Self>,
        context: &AdditionalCostContext<'_>,
        receipt: Self::Receipt,
    ) -> Result<(), AdditionalCostError> {
        params.inventories.get_mut(context.source).unwrap().bombs += receipt;
        Ok(())
    }
}

#[test]
fn configured_costs_reach_the_inventory_provider_when_the_granted_ability_activates() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(
        GameplayAbilitySystemPlugin::with_additional_costs::<InventoryProvider>(),
    );
    let mut tables = tables(vec![
        cost(1, "Inventory.Bomb", 1),
        cost(2, "Inventory.Bomb", 2),
    ]);
    let mut row = tables.tb_ability.get(&1).unwrap().as_ref().clone();
    row.allow_multiple_instances = true;
    tables.tb_ability = Arc::new(TbAbility::from_rows(vec![row]).unwrap());
    let catalog = compile_catalog(&tables, app.world_mut()).unwrap();
    let ability = Arc::clone(catalog.ability(AbilityId(1)).unwrap().definition());
    let bomb = app
        .world_mut()
        .resource_mut::<UniqueNamePool>()
        .new_name("Inventory.Bomb")
        .unwrap();
    let mut bundle = GameplayAbilitySystemBundle::default();
    let mut bindings = ConfiguredAbilities::default();
    let handle = grant_ability(
        &mut bundle.ability_system,
        &mut bindings,
        &catalog,
        AbilityId(1),
        2,
    )
    .unwrap();
    let source = app
        .world_mut()
        .spawn((
            bundle,
            bindings,
            Inventory {
                resource: bomb,
                bombs: 3,
            },
        ))
        .id();
    let mut preview =
        SystemState::<AbilityActivationCheckParams<InventoryProvider>>::new(app.world_mut());
    can_activate_ability(
        source,
        source,
        &ability,
        2,
        &preview.get(app.world()).unwrap(),
    )
    .unwrap();
    assert_eq!(app.world().get::<Inventory>(source).unwrap().bombs, 3);

    // The catalog stores requirements; the selected runtime provider supplies payment behavior.
    let mut no_provider = SystemState::<AbilityActivationCheckParams>::new(app.world_mut());
    assert_eq!(
        can_activate_ability(
            source,
            source,
            &ability,
            2,
            &no_provider.get(app.world()).unwrap()
        ),
        Err(AbilityActivationCheckError::Cost(
            AbilityCommitError::AdditionalCost(AdditionalCostError::MissingProvider)
        ))
    );
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        for _ in 0..2 {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            queue
                .push_activation(source, source, handle, context)
                .unwrap();
        }
    }
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(app.world().get::<Inventory>(source).unwrap().bombs, 0);
    let results = app.world().resource::<Messages<GameplayExecutionResult>>();
    let mut cursor = results.get_cursor();
    let outcomes: Vec<_> = cursor.read(results).map(|result| &result.outcome).collect();
    assert_eq!(outcomes.len(), 2);
    assert!(matches!(outcomes[0], GameplayExecutionOutcome::Succeeded));
    assert_eq!(
        outcomes[1],
        &GameplayExecutionOutcome::Rejected(GameplayExecutionError::AbilityActivation(
            AbilityActivationError::CommitPreparationFailed {
                source,
                handle,
                error: AbilityCommitError::AdditionalCost(
                    AdditionalCostError::InsufficientResource {
                        resource: bomb,
                        required: 3,
                        available: 0,
                    }
                ),
            }
        ))
    );
    assert_eq!(
        can_activate_ability(
            source,
            source,
            &ability,
            2,
            &preview.get(app.world()).unwrap()
        ),
        Err(AbilityActivationCheckError::Cost(
            AbilityCommitError::AdditionalCost(AdditionalCostError::InsufficientResource {
                resource: bomb,
                required: 3,
                available: 0
            })
        ))
    );
}

#[test]
fn resource_totals_and_order_positions_are_scoped_to_each_ability() {
    let mut tables = tables(vec![cost(1, "Inventory.Bomb", i64::from(u32::MAX))]);
    let first = tables.tb_ability.get(&1).unwrap().as_ref().clone();
    let mut second = first.clone();
    second.id = 2;
    tables.tb_ability = Arc::new(TbAbility::from_rows(vec![first, second]).unwrap());
    let catalog = compile_catalog(&tables, app().world_mut()).unwrap();
    let first_costs = catalog
        .ability(AbilityId(1))
        .unwrap()
        .definition()
        .get_additional_costs();
    let second_costs = catalog
        .ability(AbilityId(2))
        .unwrap()
        .definition()
        .get_additional_costs();
    assert_eq!(first_costs.len(), 1);
    assert_eq!(first_costs[0].amount(), u32::MAX);
    assert_eq!(first_costs, second_costs);
}
