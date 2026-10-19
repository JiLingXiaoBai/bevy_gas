use crate::support_test::{
    activate_ability_with_context, add_modifier, attribute_set, current_value, empty_effect_tags,
    give_ability, register_attribute, run_fixed_update, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gas::{
    AbilityActivationContext, AbilityActivationTargets, AbilityActivationTargetsError,
    AbilitySystemComponent, AbilityTags, AbilityTargetData, AbilityTargetHit, AbilityTaskDef,
    AbilityTaskOnFinishedDef, ActiveGameplayAbility, EffectDurationTicks, GameplayAbility,
    GameplayEffect, GameplayExecutionQueue, StackingPolicy,
};
use std::sync::Arc;

fn assert_approx_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= f32::EPSILON);
}

#[test]
fn activation_targets_reject_empty_acquired_target_data() {
    let empty = AbilityTargetData::new(Vec3::ZERO, Vec::new());

    assert_eq!(
        AbilityActivationTargets::acquired(empty),
        Err(AbilityActivationTargetsError::EmptyTargetData)
    );
}

#[test]
fn activation_targets_expose_one_canonical_primary_and_ordered_entities() {
    let mut world = World::new();
    let first = world.spawn_empty().id();
    let second = world.spawn_empty().id();
    let single = AbilityActivationTargets::single(first);

    assert_eq!(single.get_primary_target(), first);
    assert_eq!(single.entities().collect::<Vec<_>>(), vec![first]);
    assert!(single.get_target_data().is_none());

    let target_data = AbilityTargetData::new(
        Vec3::ZERO,
        vec![
            AbilityTargetHit::new(first, Vec3::X, None),
            AbilityTargetHit::new(second, Vec3::Y, Some(Vec3::Z)),
        ],
    );
    let acquired = AbilityActivationTargets::acquired(target_data.clone()).unwrap();

    assert_eq!(acquired.get_primary_target(), first);
    assert_eq!(acquired.entities().collect::<Vec<_>>(), vec![first, second]);
    assert_eq!(acquired.get_target_data(), Some(&target_data));
}

#[test]
fn startup_effects_apply_to_every_entity_in_target_data() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let first_attributes = attribute_set(&app, health, 100.0);
    let first = app.world_mut().spawn(first_attributes).id();
    let second_attributes = attribute_set(&app, health, 100.0);
    let second = app.world_mut().spawn(second_attributes).id();
    let damage = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -10.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![
            AbilityTaskDef::instant(AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets {
                effect: damage,
            }),
            AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
        ],
        None,
        None,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let target_data = AbilityTargetData::new(
        Vec3::ZERO,
        vec![
            AbilityTargetHit::new(first, Vec3::X, None),
            AbilityTargetHit::new(second, Vec3::new(2.0, 0.0, 0.0), None),
        ],
    );
    let context =
        AbilityActivationContext::direct(source, bevy_gas::AbilityChainContext::root(handle, 1));
    let targets = AbilityActivationTargets::acquired(target_data).unwrap();

    activate_ability_with_context(&mut app, source, targets, handle, context).unwrap();

    assert_approx_eq(current_value(&mut app, first, health), 90.0);
    assert_approx_eq(current_value(&mut app, second, health), 90.0);
}

#[test]
fn core_queue_preserves_explicit_target_forms_for_delayed_tasks_without_spatial_components() {
    for acquired in [false, true] {
        let mut app = test_app();
        let health = register_attribute(&mut app, "Health");
        let source = app
            .world_mut()
            .spawn(AbilitySystemComponent::default())
            .id();
        let first_attributes = attribute_set(&app, health, 100.0);
        let first = app.world_mut().spawn(first_attributes).id();
        let second_attributes = attribute_set(&app, health, 100.0);
        let second = app.world_mut().spawn(second_attributes).id();
        let damage = Arc::new(GameplayEffect::new(
            vec![add_modifier(health, -10.0)],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        ));
        let ability = Arc::new(GameplayAbility::new(
            AbilityTags::default(),
            vec![AbilityTaskDef::wait_ticks(
                1,
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect: damage },
            )],
            None,
            None,
            false,
        ));
        let handle = give_ability(&mut app, source, ability);
        let expected = if acquired {
            AbilityActivationTargets::acquired(AbilityTargetData::new(
                Vec3::Z,
                vec![
                    AbilityTargetHit::new(first, Vec3::X, Some(Vec3::Y)),
                    AbilityTargetHit::new(second, Vec3::Y, None),
                ],
            ))
            .unwrap()
        } else {
            AbilityActivationTargets::single(first)
        };
        {
            let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            if acquired {
                queue
                    .push_activation(source, expected.clone(), handle, context)
                    .unwrap();
            } else {
                queue
                    .push_activation(source, first, handle, context)
                    .unwrap();
            }
        }

        run_fixed_update(&mut app);

        let actual = app
            .world_mut()
            .run_system_once(move |abilities: Query<&ActiveGameplayAbility>| {
                abilities
                    .iter()
                    .find(|ability| ability.get_spec_handle() == handle)
                    .map(|ability| ability.get_targets().clone())
            })
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
        assert_approx_eq(current_value(&mut app, first, health), 100.0);
        assert_approx_eq(current_value(&mut app, second, health), 100.0);
        for entity in [source, first, second] {
            assert!(app.world().get::<Transform>(entity).is_none());
            assert!(app.world().get::<GlobalTransform>(entity).is_none());
        }

        run_fixed_update(&mut app);

        assert_approx_eq(current_value(&mut app, first, health), 90.0);
        assert_approx_eq(
            current_value(&mut app, second, health),
            if acquired { 90.0 } else { 100.0 },
        );
    }
}
