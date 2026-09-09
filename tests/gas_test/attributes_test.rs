use super::support_test::{
    active_effect_handles, add_modifier, apply_effect, apply_effect_result, attribute_set,
    current_value, empty_effect_tags, instant_add_effect, modifier, register_attribute,
    register_hot_attribute, run_effect_duration_tick, spawn_attribute_set, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gas::{
    ActiveGameplayEffects, AttributeIdManager, AttributeSet, EffectDurationTicks, GameplayEffect,
    GameplayEffectApplicationError, ModifierMagnitude, ModifierOperation, StackingPolicy,
    UniqueNamePool,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

static POST_EXECUTE_COUNT: AtomicUsize = AtomicUsize::new(0);
static EVALUATION_COUNT: AtomicUsize = AtomicUsize::new(0);

fn count_evaluations(aggregator: &bevy_gas::Aggregator, base_value: f32) -> f32 {
    EVALUATION_COUNT.fetch_add(1, Ordering::SeqCst);
    bevy_gas::default_executor(aggregator, base_value)
}

fn add_one_executor(aggregator: &bevy_gas::Aggregator, base_value: f32) -> f32 {
    bevy_gas::default_executor(aggregator, base_value) + 1.0
}

fn count_post_execute(
    _attributes: &mut AttributeSet,
    _manager: &bevy_gas::AttributeIdManager,
    _id: bevy_gas::AttributeId,
    old_value: f32,
    new_value: f32,
) {
    assert_eq!(old_value, 10.0);
    assert_eq!(new_value, 15.0);
    POST_EXECUTE_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn custom_aggregator_executor_runs_without_modifiers() {
    EVALUATION_COUNT.store(0, Ordering::SeqCst);
    let mut app = test_app();
    let health = register_hot_attribute(&mut app, "Health");
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, health, 100.0, Some(count_evaluations))
        .unwrap();

    assert_eq!(
        attributes.get_current_value(&manager, health),
        Ok(Some(100.0))
    );
    assert_eq!(
        attributes.get_current_value(&manager, health),
        Ok(Some(100.0))
    );
    assert_eq!(EVALUATION_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn attribute_id_manager_routes_ordinary_ids_to_independent_hot_and_cold_slots() {
    let mut app = test_app();
    let hot_health = register_hot_attribute(&mut app, "Health");
    let cold_strength = register_attribute(&mut app, "Strength");
    let hot_mana = register_hot_attribute(&mut app, "Mana");
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();

    assert_eq!(hot_health.to_index(), 0);
    assert_eq!(cold_strength.to_index(), 1);
    assert_eq!(hot_mana.to_index(), 2);
    assert_eq!(
        manager.location(hot_health).unwrap().region(),
        bevy_gas::AttributeRegion::Hot
    );
    assert_eq!(manager.location(hot_health).unwrap().slot(), 0);
    assert_eq!(
        manager.location(cold_strength).unwrap().region(),
        bevy_gas::AttributeRegion::Cold
    );
    assert_eq!(manager.location(cold_strength).unwrap().slot(), 0);
    assert_eq!(manager.location(hot_mana).unwrap().slot(), 1);

    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, hot_health, 100.0, None)
        .unwrap();
    attributes
        .initialize_attribute(&manager, cold_strength, 25.0, None)
        .unwrap();

    assert_eq!(
        attributes.get_current_value(&manager, hot_health),
        Ok(Some(100.0))
    );
    assert_eq!(
        attributes.get_current_value(&manager, cold_strength),
        Ok(Some(25.0))
    );
    assert_eq!(attributes.get_current_value(&manager, hot_mana), Ok(None));

    let source = app.world_mut().spawn_empty().id();
    let snapshot = attributes.make_snapshot(source);
    assert_eq!(
        snapshot.get_current_value(&manager, hot_health),
        Ok(Some(100.0))
    );
    assert_eq!(
        snapshot.get_current_value(&manager, cold_strength),
        Ok(Some(25.0))
    );
}

#[test]
fn attribute_registration_rejects_region_mismatch_and_hot_overflow() {
    let mut app = test_app();
    register_hot_attribute(&mut app, "Health");

    let mismatch = app
        .world_mut()
        .run_system_once(|mut register: bevy_gas::AttributeIdRegister| {
            register.request_or_register_attribute_id("Health", bevy_gas::AttributeRegion::Cold)
        })
        .unwrap();
    assert_eq!(
        mismatch,
        Err(bevy_gas::AttributeIdError::RegionMismatch {
            existing: bevy_gas::AttributeRegion::Hot,
            requested: bevy_gas::AttributeRegion::Cold,
        })
    );

    let overflow = app
        .world_mut()
        .run_system_once(|mut register: bevy_gas::AttributeIdRegister| {
            let mut result = Ok(());
            for index in 1..=bevy_gas::HOT_ATTRIBUTE_SET_SIZE {
                if let Err(error) = register.request_or_register_attribute_id(
                    &format!("HotAttribute{index}"),
                    bevy_gas::AttributeRegion::Hot,
                ) {
                    result = Err(error);
                    break;
                }
            }
            result
        })
        .unwrap();
    assert_eq!(
        overflow,
        Err(bevy_gas::AttributeIdError::RegionCapacityExceeded {
            region: bevy_gas::AttributeRegion::Hot,
            max: bevy_gas::HOT_ATTRIBUTE_SET_SIZE,
        })
    );
}

#[test]
fn duration_modifier_is_removed_on_expiration() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 90.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 20.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 110.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 110.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 90.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn custom_executor_survives_removal_of_last_modifier() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, health, 100.0, Some(add_one_executor))
        .unwrap();
    let target = app
        .world_mut()
        .spawn((attributes, ActiveGameplayEffects::default()))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 20.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 121.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 101.0);
}

#[test]
fn sparse_aggregators_support_reverse_location_insertion_and_independent_removal() {
    let mut app = test_app();
    let health = register_hot_attribute(&mut app, "Health");
    let mana = register_attribute(&mut app, "Mana");
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, health, 100.0, None)
        .unwrap();
    attributes
        .initialize_attribute(&manager, mana, 50.0, None)
        .unwrap();
    let target = app
        .world_mut()
        .spawn((attributes, ActiveGameplayEffects::default()))
        .id();

    let mana_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(mana, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let health_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 20.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    // Apply the later storage location first so insertion must preserve lookup correctness.
    assert!(apply_effect(&mut app, target, target, mana_effect));
    assert!(apply_effect(&mut app, target, target, health_effect));
    assert_eq!(current_value(&mut app, target, health), 120.0);
    assert_eq!(current_value(&mut app, target, mana), 60.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 100.0);
    assert_eq!(current_value(&mut app, target, mana), 60.0);
}

#[test]
fn duration_modifiers_use_add_percent_then_multiply_order() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let target = spawn_attribute_set(&mut app, damage, 100.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![
            modifier(damage, ModifierOperation::Multiply, 2.0),
            modifier(damage, ModifierOperation::PercentAdd, 0.5),
            modifier(damage, ModifierOperation::Add, 10.0),
        ],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, damage), 330.0);
}

#[test]
fn override_modifier_takes_precedence_over_other_duration_modifiers() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let target = spawn_attribute_set(&mut app, damage, 100.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![
            modifier(damage, ModifierOperation::Add, 10.0),
            modifier(damage, ModifierOperation::Multiply, 2.0),
            modifier(damage, ModifierOperation::Override, 42.0),
        ],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, damage), 42.0);
}

#[test]
fn effect_rejects_uninitialized_attribute_before_applying_any_modifier() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let mana = register_attribute(&mut app, "Mana");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0), add_modifier(mana, 5.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert_eq!(
        apply_effect_result(&mut app, target, target, effect),
        Err(GameplayEffectApplicationError::MissingAttribute { target, id: mana })
    );
    assert_eq!(current_value(&mut app, target, health), 10.0);
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    assert!(
        app.world_mut()
            .entity_mut(target)
            .get_mut::<AttributeSet>()
            .unwrap()
            .get_current_value(&manager, mana)
            .unwrap()
            .is_none()
    );
}

#[test]
fn duration_effect_rejects_uninitialized_attribute_without_becoming_active() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let mana = register_attribute(&mut app, "Mana");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(mana, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(3.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert_eq!(
        apply_effect_result(&mut app, target, target, effect),
        Err(GameplayEffectApplicationError::MissingAttribute { target, id: mana })
    );
    assert!(active_effect_handles(&app, target).is_empty());
    assert_eq!(current_value(&mut app, target, health), 10.0);
}

#[test]
fn removing_missing_modifier_handle_does_not_change_current_value() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let missing_handle = bevy_gas::ActiveEffectHandle::new(target, u32::MAX, 1);

    assert!(apply_effect(&mut app, target, target, effect));
    app.world_mut()
        .entity_mut(target)
        .get_mut::<AttributeSet>()
        .unwrap()
        .remove_modifiers(missing_handle);

    assert_eq!(current_value(&mut app, target, health), 15.0);
}

#[test]
fn instant_modifier_invokes_post_execute_callback() {
    POST_EXECUTE_COUNT.store(0, Ordering::SeqCst);
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let mut attributes = attribute_set(&app, health, 10.0);
    attributes.set_post_execute(Some(count_post_execute));
    let target = app.world_mut().spawn(attributes).id();

    assert!(apply_effect(
        &mut app,
        target,
        target,
        instant_add_effect(health, 5.0),
    ));

    assert_eq!(POST_EXECUTE_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn attribute_id_registration_reports_capacity_exceeded() {
    let mut app = test_app();
    let result = {
        let unique_names: Vec<_> = {
            let mut names = app.world_mut().resource_mut::<UniqueNamePool>();
            (0..=bevy_gas::ATTRIBUTE_SET_SIZE)
                .map(|index| names.new_name(&format!("Attribute{index}")).unwrap())
                .collect()
        };

        let mut manager = app.world_mut().resource_mut::<AttributeIdManager>();
        let mut result = Ok(());
        for (index, unique_name) in unique_names.into_iter().enumerate() {
            let region = if index < bevy_gas::HOT_ATTRIBUTE_SET_SIZE {
                bevy_gas::AttributeRegion::Hot
            } else {
                bevy_gas::AttributeRegion::Cold
            };
            if let Err(err) = manager.register_id_internal(unique_name, region) {
                result = Err(err);
                break;
            }
        }
        result
    };

    assert_eq!(
        result,
        Err(bevy_gas::AttributeIdError::CapacityExceeded {
            max: bevy_gas::ATTRIBUTE_SET_SIZE
        })
    );
}

#[test]
fn attribute_set_snapshot_captures_base_current_and_source_entity() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    assert!(apply_effect(&mut app, source, source, effect));

    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    let snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<AttributeSet>()
        .unwrap()
        .make_snapshot(source);

    assert_eq!(snapshot.get_source_entity(), source);
    assert_eq!(snapshot.get_base_value(&manager, health), Ok(Some(10.0)));
    assert_eq!(snapshot.get_current_value(&manager, health), Ok(Some(15.0)));
}

#[test]
fn attribute_set_snapshot_is_not_changed_by_later_attribute_mutation() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = spawn_attribute_set(&mut app, health, 10.0);
    let manager = app
        .world()
        .resource::<bevy_gas::AttributeIdManager>()
        .clone();
    let snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<AttributeSet>()
        .unwrap()
        .make_snapshot(source);

    assert!(apply_effect(
        &mut app,
        source,
        source,
        instant_add_effect(health, 20.0),
    ));

    assert_eq!(current_value(&mut app, source, health), 30.0);
    assert_eq!(snapshot.get_base_value(&manager, health), Ok(Some(10.0)));
    assert_eq!(snapshot.get_current_value(&manager, health), Ok(Some(10.0)));
}

#[test]
fn attribute_location_reports_manager_mismatch() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let empty_manager = AttributeIdManager::default();

    assert_eq!(
        empty_manager.location(health),
        Err(bevy_gas::AttributeIdError::MissingLocation { id: health })
    );
}
