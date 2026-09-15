use bevy::prelude::{App, MinimalPlugins, World};
use bevy_gas::config::generated::gas::{
    Ability, AbilityTask, ActionKind, Attribute, AttributeRegion, DurationKind, Effect,
    SelectionKind, SortOrder, Tag, TargetScope, Targeting, TbAbility, TbAbilityTask, TbAttribute,
    TbEffect, TbTag, TbTargeting,
};
use bevy_gas::config::generated::{ByteBuf, Tables};
use bevy_gas::config::{
    AbilityId, ConfigErrorKind, ConfigLocation, EffectId, GameplayCatalog, compile_catalog,
};
#[cfg(feature = "config-validation")]
use bevy_gas::config::{describe_ability, validate_tables};
use bevy_gas::{
    AbilityTaskDef, AbilityTaskOnFinishedDef, AttributeIdManager,
    AttributeRegion as RuntimeAttributeRegion, GameplayAbilitySystemPlugin, GameplayTagManager,
    UniqueNamePool,
};
use std::sync::Arc;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(GameplayAbilitySystemPlugin);
    app
}

fn empty_tables() -> Tables {
    Tables::new(|_| Ok(ByteBuf::new(vec![0]))).unwrap()
}

fn configured_names() -> Tables {
    let mut tables = empty_tables();
    tables.tb_tag = Arc::new(
        TbTag::from_rows(vec![Tag {
            name: "AddedTag".to_owned(),
            description: String::new(),
        }])
        .unwrap(),
    );
    tables.tb_attribute = Arc::new(
        TbAttribute::from_rows(vec![
            Attribute {
                name: "AAdded".to_owned(),
                region: AttributeRegion::Hot,
                description: String::new(),
            },
            Attribute {
                name: "ZExisting".to_owned(),
                region: AttributeRegion::Cold,
                description: String::new(),
            },
        ])
        .unwrap(),
    );
    tables
}

fn assert_unchanged(world: &World, names: &UniqueNamePool, attributes: &AttributeIdManager) {
    // Probe cloned pools so the assertion itself never mutates either registry.
    let mut expected_names = names.clone();
    let mut actual_names = world.resource::<UniqueNamePool>().clone();
    assert_eq!(
        expected_names.new_name("AfterFailure").unwrap(),
        actual_names.new_name("AfterFailure").unwrap()
    );
    let added_name = actual_names.new_name("AddedTag").unwrap();
    assert!(
        world
            .resource::<GameplayTagManager>()
            .get_tag(added_name)
            .is_none()
    );
    let actual_attributes = world.resource::<AttributeIdManager>();
    assert_eq!(actual_attributes.hot_count(), attributes.hot_count());
    assert_eq!(actual_attributes.cold_count(), attributes.cold_count());
    let existing_name = actual_names.new_name("ZExisting").unwrap();
    assert_eq!(
        actual_attributes.get_attribute_id(existing_name),
        attributes.get_attribute_id(existing_name)
    );
}

#[test]
fn existing_registry_conflict_preserves_state_and_retry_matches_clean_compilation() {
    let mut app = app();
    let existing_name = app
        .world_mut()
        .resource_mut::<UniqueNamePool>()
        .new_name("ZExisting")
        .unwrap();
    app.world_mut()
        .resource_mut::<AttributeIdManager>()
        .register_id_internal(existing_name, RuntimeAttributeRegion::Hot)
        .unwrap();
    let names = app.world().resource::<UniqueNamePool>().clone();
    let attributes = app.world().resource::<AttributeIdManager>().clone();
    let tags = app.world().resource::<GameplayTagManager>().clone();

    let mut tables = configured_names();
    let error = compile_catalog(&tables, app.world_mut()).err().unwrap();
    assert_eq!(error.kind(), ConfigErrorKind::Registration);
    assert!(
        matches!(error.location(), ConfigLocation::Table { table: "Attribute", row: Some(row), .. } if row == "ZExisting")
    );
    assert_unchanged(app.world(), &names, &attributes);

    // Correct the existing name's region, then compare against a clean seeded World.
    let rows = tables
        .tb_attribute
        .iter()
        .map(|row| {
            let mut row = row.as_ref().clone();
            row.region = AttributeRegion::Hot;
            row
        })
        .collect();
    tables.tb_attribute = Arc::new(TbAttribute::from_rows(rows).unwrap());
    let recovered = compile_catalog(&tables, app.world_mut()).unwrap();
    let mut clean = World::new();
    clean.insert_resource(names);
    clean.insert_resource(attributes);
    clean.insert_resource(tags);
    let expected = compile_catalog(&tables, &mut clean).unwrap();
    assert_eq!(recovered.tag("AddedTag"), expected.tag("AddedTag"));
    assert_eq!(recovered.attribute("AAdded"), expected.attribute("AAdded"));
    assert_eq!(
        recovered.attribute("ZExisting"),
        expected.attribute("ZExisting")
    );
}

#[test]
fn construction_failure_preserves_registries_and_existing_catalog() {
    let mut app = app();
    let published = compile_catalog(&empty_tables(), app.world_mut()).unwrap();
    app.world_mut().insert_resource(published);
    let names = app.world().resource::<UniqueNamePool>().clone();
    let attributes = app.world().resource::<AttributeIdManager>().clone();
    let mut tables = configured_names();
    tables.tb_ability = Arc::new(TbAbility::from_rows(vec![ability()]).unwrap());
    // No targeting row exists. Without authoring validation this fails after registration.
    assert!(compile_catalog(&tables, app.world_mut()).is_err());
    assert_unchanged(app.world(), &names, &attributes);
    assert_eq!(
        app.world()
            .resource::<GameplayCatalog>()
            .abilities()
            .count(),
        0
    );
}

#[test]
fn missing_registry_is_classified_and_does_not_remove_other_resources() {
    let mut world = World::new();
    world.insert_resource(UniqueNamePool::default());
    world.insert_resource(GameplayTagManager::default());
    let error = compile_catalog(&empty_tables(), &mut world).err().unwrap();
    assert_eq!(error.kind(), ConfigErrorKind::MissingResource);
    assert_eq!(
        error.location(),
        &ConfigLocation::Resource("AttributeIdManager")
    );
    assert!(world.contains_resource::<UniqueNamePool>());
    assert!(world.contains_resource::<GameplayTagManager>());
}

fn ability() -> Ability {
    Ability {
        id: 1,
        name: "Timeline".to_owned(),
        max_level: 2,
        cost_effect_id: None,
        cooldown_effect_id: None,
        activation_effect_ids: vec![],
        asset_tags: vec![],
        required_tags: vec![],
        blocked_tags: vec![],
        cancel_ability_tags: vec![],
        block_ability_tags: vec![],
        targeting_id: 1,
        end_on_activation: false,
        allow_multiple_instances: false,
    }
}

fn timeline_tables(reverse: bool) -> Tables {
    let mut tables = empty_tables();
    tables.tb_ability = Arc::new(TbAbility::from_rows(vec![ability()]).unwrap());
    tables.tb_targeting = Arc::new(
        TbTargeting::from_rows(vec![Targeting {
            id: 1,
            name: "Self".to_owned(),
            selection: SelectionKind::SelfTarget,
            radius: None,
            half_angle_radians: None,
            max_distance: None,
            exclude_source: false,
            require_attributes: false,
            required_tags: vec![],
            blocked_tags: vec![],
            sort: SortOrder::None,
            limit: 1,
        }])
        .unwrap(),
    );
    tables.tb_effect = Arc::new(
        TbEffect::from_rows(
            (1..=3)
                .map(|id| Effect {
                    id,
                    name: format!("Effect {id}"),
                    duration_kind: DurationKind::Instant,
                    duration_ticks: None,
                    period_ticks: None,
                    execute_on_applied: false,
                    probability: 1.0,
                    asset_tags: vec![],
                    granted_tags: vec![],
                })
                .collect(),
        )
        .unwrap(),
    );
    let mut actions = vec![
        AbilityTask {
            id: 4,
            ability_id: 1,
            at_tick: 2,
            order: 3,
            kind: ActionKind::EndAbility,
            target_scope: TargetScope::None,
            effect_id: None,
        },
        AbilityTask {
            id: 2,
            ability_id: 1,
            at_tick: 2,
            order: 1,
            kind: ActionKind::ApplyEffect,
            target_scope: TargetScope::AllCaptured,
            effect_id: Some(2),
        },
        AbilityTask {
            id: 1,
            ability_id: 1,
            at_tick: 0,
            order: 0,
            kind: ActionKind::ApplyEffect,
            target_scope: TargetScope::Primary,
            effect_id: Some(1),
        },
        AbilityTask {
            id: 3,
            ability_id: 1,
            at_tick: 2,
            order: 2,
            kind: ActionKind::ApplyEffect,
            target_scope: TargetScope::Primary,
            effect_id: Some(3),
        },
    ];
    if reverse {
        actions.reverse();
    }
    tables.tb_ability_task = Arc::new(TbAbilityTask::from_rows(actions).unwrap());
    tables
}

#[test]
fn authored_order_drives_compiled_tasks_independently_of_input_row_order() {
    for reverse in [false, true] {
        let tables = timeline_tables(reverse);
        let catalog = compile_catalog(&tables, app().world_mut()).unwrap();
        let tasks = catalog
            .ability(AbilityId(1))
            .unwrap()
            .definition()
            .get_startup_tasks();
        assert_eq!(tasks.len(), 2);
        let AbilityTaskDef::Instant {
            on_finished: AbilityTaskOnFinishedDef::Batch { actions: startup },
        } = &tasks[0]
        else {
            panic!("expected one startup action batch");
        };
        assert_eq!(startup.len(), 1);
        assert!(
            matches!(&startup[0], AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect } if Arc::ptr_eq(effect, catalog.effect(EffectId(1)).unwrap()))
        );
        let AbilityTaskDef::WaitTicks {
            ticks: 2,
            on_finished: AbilityTaskOnFinishedDef::Batch { actions: delayed },
        } = &tasks[1]
        else {
            panic!("expected one batch at tick two");
        };
        assert_eq!(delayed.len(), 3);
        assert!(
            matches!(&delayed[0], AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect } if Arc::ptr_eq(effect, catalog.effect(EffectId(2)).unwrap()))
        );
        assert!(
            matches!(&delayed[1], AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect } if Arc::ptr_eq(effect, catalog.effect(EffectId(3)).unwrap()))
        );
        assert!(matches!(&delayed[2], AbilityTaskOnFinishedDef::EndAbility));
    }
}

#[cfg(feature = "config-validation")]
#[test]
fn validation_and_inspection_share_the_compiler_timeline_order() {
    let tables = timeline_tables(false);
    validate_tables(&tables).unwrap();
    let report = describe_ability(&tables, AbilityId(1)).unwrap();
    assert_eq!(
        report,
        describe_ability(&timeline_tables(true), AbilityId(1)).unwrap()
    );
    let actions: Vec<_> = report
        .lines()
        .filter(|line| line.starts_with("tick "))
        .collect();
    assert_eq!(
        actions,
        [
            "tick 0 / order 0: ApplyEffect to Primary",
            "tick 2 / order 1: ApplyEffect to AllCaptured",
            "tick 2 / order 2: ApplyEffect to Primary",
            "tick 2 / order 3: EndAbility",
        ]
    );
}
