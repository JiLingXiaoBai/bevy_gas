use crate::support_test::{
    activate_ability_with_context, add_modifier, add_tag_to_entity, attribute_set, current_value,
    empty_effect_tags, give_ability, register_attribute, register_tag, remove_tag_from_entity,
    run_fixed_update, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_gas::{
    AbilityActivationContext, AbilityActivationTargets, AbilityActivationTargetsError,
    AbilitySystemComponent, AbilityTags, AbilityTargetData, AbilityTargetHit, AbilityTaskDef,
    AbilityTaskOnFinishedDef, EffectDurationTicks, GameplayAbility, GameplayAbilitySystemSet,
    GameplayEffect, GameplayTagContainer, LogicalPosition, PositionPresentation, StackingPolicy,
    TagRequirements, Targetable, TargetingContinuation, TargetingDefinition,
    TargetingDefinitionError, TargetingError, TargetingInput, TargetingOperation,
    TargetingRequestQueue, TargetingResultEvent, TargetingSortOrder, TargetingSpatialIndex,
    TargetingSystemParams, acquire_targets, sync_targeting_spatial_index_system,
};
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn targeting_definition(operations: Vec<TargetingOperation>) -> Arc<TargetingDefinition> {
    Arc::new(TargetingDefinition::new(operations).unwrap())
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

fn assert_approx_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= f32::EPSILON);
}

fn spawn_targetable(app: &mut App, position: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            Targetable,
            LogicalPosition(position),
            GameplayTagContainer::default(),
        ))
        .id()
}

fn set_target_position(app: &mut App, target: Entity, position: Vec3) {
    app.world_mut()
        .get_mut::<LogicalPosition>(target)
        .unwrap()
        .0 = position;
}

fn acquire(
    app: &mut App,
    source: Entity,
    input: TargetingInput,
    definition: Arc<TargetingDefinition>,
) -> Result<AbilityTargetData, TargetingError> {
    app.world_mut()
        .run_system_once(sync_targeting_spatial_index_system)
        .unwrap();
    acquire_current(app, source, input, definition)
}

fn acquire_current(
    app: &mut App,
    source: Entity,
    input: TargetingInput,
    definition: Arc<TargetingDefinition>,
) -> Result<AbilityTargetData, TargetingError> {
    app.world_mut()
        .run_system_once(move |params: TargetingSystemParams| {
            acquire_targets(source, input, &definition, &params)
        })
        .unwrap()
}

fn assert_current_targets(
    app: &mut App,
    source: Entity,
    definition: &Arc<TargetingDefinition>,
    expected: &[Entity],
) {
    let result = acquire_current(
        app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        Arc::clone(definition),
    );
    if expected.is_empty() {
        assert_eq!(result, Err(TargetingError::NoTargetsFound));
    } else {
        assert_eq!(result.unwrap().entities().collect::<Vec<_>>(), expected);
    }
}

#[test]
fn targeting_definition_rejects_invalid_pipelines() {
    assert!(matches!(
        TargetingDefinition::new(Vec::new()),
        Err(TargetingDefinitionError::EmptyOperations)
    ));
    assert!(matches!(
        TargetingDefinition::new(vec![TargetingOperation::FilterSource]),
        Err(TargetingDefinitionError::SelectionMustBeFirst)
    ));
    assert!(matches!(
        TargetingDefinition::new(vec![
            TargetingOperation::SelectSelf,
            TargetingOperation::SelectSphere { radius: 1.0 },
        ]),
        Err(TargetingDefinitionError::MultipleSelections)
    ));
    assert!(matches!(
        TargetingDefinition::new(vec![TargetingOperation::SelectSphere { radius: -1.0 }]),
        Err(TargetingDefinitionError::InvalidRadius { .. })
    ));
    assert!(matches!(
        TargetingDefinition::new(vec![
            TargetingOperation::SelectSelf,
            TargetingOperation::Limit { count: 0 },
        ]),
        Err(TargetingDefinitionError::ZeroLimit)
    ));
}

#[test]
fn sphere_targeting_filters_sorts_and_limits_deterministically() {
    let mut app = test_app();
    let enemy = register_tag(&mut app, "Team.Enemy");
    let dead = register_tag(&mut app, "State.Dead");
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let near = spawn_targetable(&mut app, Vec3::new(1.0, 0.0, 0.0));
    let equal_a = spawn_targetable(&mut app, Vec3::new(2.0, 0.0, 0.0));
    let equal_b = spawn_targetable(&mut app, Vec3::new(-2.0, 0.0, 0.0));
    let dead_enemy = spawn_targetable(&mut app, Vec3::new(0.5, 0.0, 0.0));
    let neutral = spawn_targetable(&mut app, Vec3::new(0.25, 0.0, 0.0));

    for target in [near, equal_a, equal_b, dead_enemy] {
        add_tag_to_entity(&mut app, target, enemy);
    }
    add_tag_to_entity(&mut app, dead_enemy, dead);

    let requirements = TagRequirements::new(vec![enemy], vec![dead]).unwrap();
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 3.0 },
        TargetingOperation::FilterTags { requirements },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
        TargetingOperation::Limit { count: 3 },
    ]);

    let result = acquire(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        definition,
    )
    .unwrap();
    let entities = result.entities().collect::<Vec<_>>();
    let (first_equal, second_equal) = if equal_a.to_bits() < equal_b.to_bits() {
        (equal_a, equal_b)
    } else {
        (equal_b, equal_a)
    };

    assert_eq!(entities, vec![near, first_equal, second_equal]);
    assert!(!entities.contains(&dead_enemy));
    assert!(!entities.contains(&neutral));
}

#[test]
fn cone_targeting_uses_direction_and_rejects_zero_direction() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let forward = spawn_targetable(&mut app, Vec3::new(2.0, 0.0, 0.0));
    let diagonal = spawn_targetable(&mut app, Vec3::new(2.0, 0.0, 2.0));
    let behind = spawn_targetable(&mut app, Vec3::new(-1.0, 0.0, 0.0));
    let definition = targeting_definition(vec![TargetingOperation::SelectCone {
        radius: 3.0,
        half_angle_radians: std::f32::consts::FRAC_PI_4,
    }]);

    let result = acquire(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        definition.clone(),
    )
    .unwrap();
    let entities = result.entities().collect::<Vec<_>>();
    assert!(entities.contains(&forward));
    assert!(entities.contains(&diagonal));
    assert!(!entities.contains(&behind));

    assert_eq!(
        acquire(
            &mut app,
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::ZERO),
            definition,
        ),
        Err(TargetingError::InvalidDirection)
    );
}

#[test]
fn explicit_target_requires_targetable_marker() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let unmarked = app.world_mut().spawn(LogicalPosition(Vec3::X)).id();
    let definition = targeting_definition(vec![TargetingOperation::SelectExplicitEntity]);

    assert_eq!(
        acquire(
            &mut app,
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(unmarked),
            definition,
        ),
        Err(TargetingError::TargetNotTargetable { target: unmarked })
    );
}

#[test]
fn explicit_target_can_be_rejected_by_distance() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let target = spawn_targetable(&mut app, Vec3::new(5.0, 0.0, 0.0));
    let definition = targeting_definition(vec![
        TargetingOperation::SelectExplicitEntity,
        TargetingOperation::FilterDistance { max_distance: 4.0 },
    ]);

    assert_eq!(
        acquire(
            &mut app,
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(target),
            definition,
        ),
        Err(TargetingError::NoTargetsFound)
    );
}

#[test]
fn targeting_queue_activates_ability_with_complete_target_data() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn((
            AbilitySystemComponent::default(),
            LogicalPosition(Vec3::ZERO),
        ))
        .id();
    let target = spawn_targetable(&mut app, Vec3::X);
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let definition = targeting_definition(vec![TargetingOperation::SelectExplicitEntity]);

    let context = {
        let mut queue = app
            .world_mut()
            .resource_mut::<bevy_gas::GameplayExecutionQueue>();
        AbilityActivationContext::direct(source, queue.new_root_chain(handle))
    };
    app.world_mut()
        .resource_mut::<TargetingRequestQueue>()
        .push_request(
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(target),
            definition,
            TargetingContinuation::activate_ability(handle, context),
        );

    run_fixed_update(&mut app);

    let targets = app
        .world_mut()
        .run_system_once(move |query: Query<&bevy_gas::ActiveGameplayAbility>| {
            query
                .iter()
                .find(|active| active.get_spec_handle() == handle)
                .map(|active| active.get_targets().clone())
        })
        .unwrap()
        .unwrap();
    assert_eq!(targets.get_primary_target(), target);
    assert_eq!(
        targets.get_target_data().map(AbilityTargetData::get_hits),
        Some(&[AbilityTargetHit::new(target, Vec3::X, None)][..])
    );
}

#[test]
fn targeting_queue_processes_entire_batch() {
    const REQUEST_COUNT: usize = 33;

    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let definition = targeting_definition(vec![TargetingOperation::SelectSelf]);
    {
        let mut queue = app.world_mut().resource_mut::<TargetingRequestQueue>();
        for _ in 0..REQUEST_COUNT {
            queue.push_request(
                source,
                TargetingInput::new(Vec3::ZERO, Vec3::X),
                definition.clone(),
                TargetingContinuation::EmitResult,
            );
        }
    }

    run_fixed_update(&mut app);
    assert!(app.world().resource::<TargetingRequestQueue>().is_empty());
}

#[test]
fn multi_target_task_applies_effect_to_every_acquired_entity() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = app
        .world_mut()
        .spawn((
            AbilitySystemComponent::default(),
            LogicalPosition(Vec3::ZERO),
        ))
        .id();
    let first_attributes = attribute_set(&app, health, 100.0);
    let first = app
        .world_mut()
        .spawn((Targetable, LogicalPosition(Vec3::X), first_attributes))
        .id();
    let second_attributes = attribute_set(&app, health, 100.0);
    let second = app
        .world_mut()
        .spawn((
            Targetable,
            LogicalPosition(Vec3::new(2.0, 0.0, 0.0)),
            second_attributes,
        ))
        .id();
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
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect: damage },
        )],
        None,
        None,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 3.0 },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
    ]);
    let context = {
        let mut queue = app
            .world_mut()
            .resource_mut::<bevy_gas::GameplayExecutionQueue>();
        AbilityActivationContext::direct(source, queue.new_root_chain(handle))
    };
    app.world_mut()
        .resource_mut::<TargetingRequestQueue>()
        .push_request(
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X),
            definition,
            TargetingContinuation::activate_ability(handle, context),
        );

    run_fixed_update(&mut app);

    assert_approx_eq(current_value(&mut app, first, health), 90.0);
    assert_approx_eq(current_value(&mut app, second, health), 90.0);
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
fn range_targeting_requires_sync_without_falling_back_to_scanning() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let target = spawn_targetable(&mut app, Vec3::X);
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);
    let input = TargetingInput::new(Vec3::ZERO, Vec3::X);

    assert_eq!(
        acquire_current(&mut app, source, input, Arc::clone(&sphere)),
        Err(TargetingError::SpatialIndexNotReady)
    );
    let explicit = targeting_definition(vec![TargetingOperation::SelectExplicitEntity]);
    let self_selection = targeting_definition(vec![TargetingOperation::SelectSelf]);
    assert_eq!(
        acquire_current(
            &mut app,
            source,
            input.with_explicit_target(target),
            explicit
        )
        .unwrap()
        .primary_entity(),
        Some(target)
    );
    assert_eq!(
        acquire_current(&mut app, source, input, self_selection)
            .unwrap()
            .primary_entity(),
        Some(source)
    );

    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);
}

#[test]
fn empty_queue_ticks_initialize_and_update_the_spatial_index() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);

    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[]);
    let target = spawn_targetable(&mut app, Vec3::X);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);
    assert!(app.world().resource::<TargetingRequestQueue>().is_empty());
}

#[test]
fn sphere_and_cone_reject_aabb_candidates_outside_the_exact_shape() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let boundary = spawn_targetable(&mut app, Vec3::X);
    let corner = spawn_targetable(&mut app, Vec3::new(0.9, 0.9, 0.0));
    let outside = spawn_targetable(&mut app, Vec3::new(1.001, 0.0, 0.0));
    let side = spawn_targetable(&mut app, Vec3::new(0.1, 0.9, 0.0));
    let behind = spawn_targetable(&mut app, -Vec3::X);
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 1.0 }]);
    let cone = targeting_definition(vec![TargetingOperation::SelectCone {
        radius: 1.0,
        half_angle_radians: std::f32::consts::FRAC_PI_4,
    }]);
    run_fixed_update(&mut app);

    let mut expected_sphere = vec![boundary, side, behind];
    expected_sphere.sort_unstable_by_key(|entity| entity.to_bits());
    assert_current_targets(&mut app, source, &sphere, &expected_sphere);
    assert_current_targets(&mut app, source, &cone, &[boundary]);
    assert!(!expected_sphere.contains(&corner));
    assert!(!expected_sphere.contains(&outside));
}

#[test]
fn zero_radius_selects_only_the_exact_position() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let exact = spawn_targetable(&mut app, Vec3::ZERO);
    spawn_targetable(&mut app, Vec3::new(f32::EPSILON, 0.0, 0.0));
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 0.0 }]);
    let cone = targeting_definition(vec![TargetingOperation::SelectCone {
        radius: 0.0,
        half_angle_radians: 0.0,
    }]);
    run_fixed_update(&mut app);

    assert_current_targets(&mut app, source, &sphere, &[exact]);
    assert_current_targets(&mut app, source, &cone, &[exact]);
}

#[test]
fn finite_extreme_positions_do_not_overflow_range_or_distance_checks() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let origin = Vec3::new(-f32::MAX, 0.0, 0.0);
    let exact = spawn_targetable(&mut app, origin);
    let boundary = spawn_targetable(&mut app, Vec3::ZERO);
    let outside = spawn_targetable(&mut app, Vec3::new(f32::MAX, 0.0, 0.0));
    let sphere = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: f32::MAX },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
    ]);
    let result = acquire(
        &mut app,
        source,
        TargetingInput::new(origin, Vec3::X),
        sphere,
    )
    .unwrap();
    assert_eq!(result.entities().collect::<Vec<_>>(), vec![exact, boundary]);

    let explicit = targeting_definition(vec![
        TargetingOperation::SelectExplicitEntity,
        TargetingOperation::FilterDistance {
            max_distance: f32::MAX,
        },
    ]);
    assert_eq!(
        acquire_current(
            &mut app,
            source,
            TargetingInput::new(origin, Vec3::X).with_explicit_target(outside),
            explicit,
        ),
        Err(TargetingError::NoTargetsFound)
    );
}

#[test]
fn incremental_sync_tracks_motion_component_changes_and_entity_replacement() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let target = spawn_targetable(&mut app, Vec3::ZERO);
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 0.1 }]);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    let small_motion = Vec3::new(0.05, 0.0, 0.0);
    set_target_position(&mut app, target, small_motion);
    run_fixed_update(&mut app);
    let moved = acquire_current(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        Arc::clone(&sphere),
    )
    .unwrap();
    assert_eq!(
        moved.get_hits(),
        &[AbilityTargetHit::new(target, small_motion, None)]
    );

    set_target_position(&mut app, target, Vec3::splat(30.0));
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[]);
    set_target_position(&mut app, target, Vec3::ZERO);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    app.world_mut().entity_mut(target).remove::<Targetable>();
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[]);
    app.world_mut().entity_mut(target).insert(Targetable);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    app.world_mut()
        .entity_mut(target)
        .remove::<LogicalPosition>();
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[]);
    app.world_mut()
        .entity_mut(target)
        .insert(LogicalPosition(Vec3::ZERO));
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    app.world_mut()
        .entity_mut(target)
        .remove::<Targetable>()
        .insert(Targetable);
    app.world_mut()
        .entity_mut(target)
        .remove::<LogicalPosition>()
        .insert(LogicalPosition(Vec3::ZERO));
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    assert!(app.world_mut().despawn(target));
    let replacement = spawn_targetable(&mut app, Vec3::ZERO);
    assert_ne!(target, replacement);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[replacement]);
}

#[test]
fn nonfinite_positions_are_excluded_and_recover_when_moved_back() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let target = spawn_targetable(&mut app, Vec3::ZERO);
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 1.0 }]);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &sphere, &[target]);

    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        set_target_position(&mut app, target, Vec3::new(invalid, 0.0, 0.0));
        run_fixed_update(&mut app);
        assert_current_targets(&mut app, source, &sphere, &[]);
        let explicit = targeting_definition(vec![TargetingOperation::SelectExplicitEntity]);
        assert_eq!(
            acquire_current(
                &mut app,
                source,
                TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(target),
                explicit,
            ),
            Err(TargetingError::InvalidTargetPosition { target })
        );
        let self_selection = targeting_definition(vec![TargetingOperation::SelectSelf]);
        assert_eq!(
            acquire_current(
                &mut app,
                target,
                TargetingInput::new(Vec3::ZERO, Vec3::X),
                self_selection,
            ),
            Err(TargetingError::InvalidTargetPosition { target })
        );
        set_target_position(&mut app, target, Vec3::ZERO);
        run_fixed_update(&mut app);
        assert_current_targets(&mut app, source, &sphere, &[target]);
    }
}

#[test]
fn equal_distance_order_is_stable_after_movement_and_reinsertion() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let positions = [Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y];
    let targets = positions.map(|position| spawn_targetable(&mut app, position));
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 2.0 },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
        TargetingOperation::Limit { count: 2 },
    ]);
    let mut expected = targets;
    expected.sort_unstable_by_key(|entity| entity.to_bits());
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &definition, &expected[..2]);

    for target in targets.iter().rev() {
        set_target_position(&mut app, *target, Vec3::splat(100.0));
        run_fixed_update(&mut app);
    }
    for (target, position) in targets.iter().zip(positions).rev() {
        app.world_mut().entity_mut(*target).remove::<Targetable>();
        run_fixed_update(&mut app);
        app.world_mut()
            .entity_mut(*target)
            .insert((Targetable, LogicalPosition(position)));
        run_fixed_update(&mut app);
    }
    assert_current_targets(&mut app, source, &definition, &expected[..2]);
}

#[derive(Resource, Default)]
struct CapturedTargetingResults(Vec<Result<AbilityTargetData, TargetingError>>);

#[test]
fn request_producer_motion_and_deferred_spawns_are_indexed_in_the_same_tick() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let moving = spawn_targetable(&mut app, Vec3::splat(100.0));
    run_fixed_update(&mut app);
    app.init_resource::<CapturedTargetingResults>();
    app.world_mut().add_observer(
        |event: On<TargetingResultEvent>, mut captured: ResMut<CapturedTargetingResults>| {
            captured.0.push(event.get_result().clone());
        },
    );
    let definition = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);
    app.add_systems(
        FixedUpdate,
        (move |mut commands: Commands,
               mut positions: Query<&mut LogicalPosition>,
               mut queue: ResMut<TargetingRequestQueue>| {
            positions.get_mut(moving).unwrap().0 = Vec3::X;
            commands.spawn((Targetable, LogicalPosition(-Vec3::X)));
            queue.push_request(
                source,
                TargetingInput::new(Vec3::ZERO, Vec3::X),
                Arc::clone(&definition),
                TargetingContinuation::EmitResult,
            );
        })
        .in_set(GameplayAbilitySystemSet::RequestProducers),
    );

    run_fixed_update(&mut app);

    let captured = app.world().resource::<CapturedTargetingResults>();
    assert_eq!(captured.0.len(), 1);
    let data = captured.0[0].as_ref().unwrap();
    assert_eq!(data.get_hits().len(), 2);
    assert!(data.entities().any(|entity| entity == moving));
    assert!(
        data.get_hits()
            .contains(&AbilityTargetHit::new(moving, Vec3::X, None))
    );
}

#[test]
fn repeated_dynamic_updates_preserve_the_expected_target_set() {
    const TARGET_COUNT: usize = 96;
    const NEAR_COUNT: usize = 8;
    const TICK_COUNT: usize = 48;

    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let mut targets = (0..TARGET_COUNT)
        .map(|index| spawn_targetable(&mut app, Vec3::new(index as f32 + 100.0, 0.0, 0.0)))
        .collect::<Vec<_>>();
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 10.0 },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
    ]);
    run_fixed_update(&mut app);

    for tick in 0..TICK_COUNT {
        let replaced = tick % TARGET_COUNT;
        assert!(app.world_mut().despawn(targets[replaced]));
        targets[replaced] = spawn_targetable(&mut app, Vec3::splat(200.0));
        for (index, target) in targets.iter().copied().enumerate() {
            let rank = (index + TARGET_COUNT - tick) % TARGET_COUNT;
            let distance = if rank < NEAR_COUNT {
                rank as f32 + 1.0
            } else {
                rank as f32 + 100.0
            };
            let axis = if tick % 2 == 0 { Vec3::X } else { -Vec3::Y };
            set_target_position(&mut app, target, axis * distance);
        }
        run_fixed_update(&mut app);
        let expected = (0..NEAR_COUNT)
            .map(|rank| targets[(tick + rank) % TARGET_COUNT])
            .collect::<Vec<_>>();
        assert_current_targets(&mut app, source, &definition, &expected);
    }
}
#[test]
fn removed_entities_leave_no_stale_proxies_when_removal_events_expire() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let marker_removed = spawn_targetable(&mut app, Vec3::X);
    let position_removed = spawn_targetable(&mut app, Vec3::Y);
    let despawned = spawn_targetable(&mut app, Vec3::Z);
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);
    run_fixed_update(&mut app);
    assert_eq!(app.world().resource::<TargetingSpatialIndex>().len(), 3);

    app.world_mut()
        .entity_mut(marker_removed)
        .remove::<Targetable>();
    app.world_mut()
        .entity_mut(position_removed)
        .remove::<LogicalPosition>();
    assert!(app.world_mut().despawn(despawned));
    app.world_mut().clear_trackers();
    run_fixed_update(&mut app);

    assert_current_targets(&mut app, source, &sphere, &[]);
    assert!(app.world().resource::<TargetingSpatialIndex>().is_empty());
    app.world_mut()
        .entity_mut(marker_removed)
        .insert(Targetable);
    app.world_mut()
        .entity_mut(position_removed)
        .insert(LogicalPosition(Vec3::ZERO));
    let replacement = spawn_targetable(&mut app, Vec3::ZERO);
    run_fixed_update(&mut app);
    let mut expected = vec![marker_removed, position_removed, replacement];
    expected.sort_unstable_by_key(|entity| entity.to_bits());
    assert_current_targets(&mut app, source, &sphere, &expected);
    assert_eq!(app.world().resource::<TargetingSpatialIndex>().len(), 3);
}

#[test]
#[ignore = "Measures dynamic AABB maintenance and queries; run explicitly with --nocapture."]
fn measure_dynamic_aabb_targeting_workloads() {
    const MEASURED_TICKS: usize = 4;

    for target_count in [64, 4096] {
        for radius in [12.0, 512.0] {
            for moving_percent in [0, 10, 100] {
                for requests_per_tick in [1, 32] {
                    let mut app = test_app();
                    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
                    let targets = (0..target_count)
                        .map(|index| {
                            let position = Vec3::new(
                                (index % 64) as f32 * 4.0,
                                (index / 64) as f32 * 4.0,
                                0.0,
                            );
                            (spawn_targetable(&mut app, position), position)
                        })
                        .collect::<Vec<_>>();
                    let definition = targeting_definition(vec![
                        TargetingOperation::SelectSphere { radius },
                        TargetingOperation::SortByDistance {
                            order: TargetingSortOrder::Ascending,
                        },
                    ]);
                    let queries =
                        app.world_mut()
                            .register_system(move |params: TargetingSystemParams| {
                                for _ in 0..requests_per_tick {
                                    let result = acquire_targets(
                                        source,
                                        TargetingInput::new(Vec3::ZERO, Vec3::X),
                                        &definition,
                                        &params,
                                    )
                                    .unwrap();
                                    black_box(result);
                                }
                            });
                    run_fixed_update(&mut app);
                    app.world_mut().run_system(queries).unwrap();
                    let moving_count = target_count * moving_percent / 100;
                    let mut maintenance_elapsed = Duration::ZERO;
                    let mut queries_elapsed = Duration::ZERO;
                    for tick in 0..MEASURED_TICKS {
                        let displacement = Vec3::Z * if tick % 2 == 0 { 0.25 } else { -0.25 };
                        for &(target, base_position) in targets.iter().take(moving_count) {
                            set_target_position(&mut app, target, base_position + displacement);
                        }
                        let started = Instant::now();
                        run_fixed_update(&mut app);
                        maintenance_elapsed += started.elapsed();
                        let started = Instant::now();
                        app.world_mut().run_system(queries).unwrap();
                        queries_elapsed += started.elapsed();
                    }
                    let milliseconds_per_tick = 1000.0 / MEASURED_TICKS as f64;
                    let maintenance_ms = maintenance_elapsed.as_secs_f64() * milliseconds_per_tick;
                    let queries_ms = queries_elapsed.as_secs_f64() * milliseconds_per_tick;
                    println!(
                        "targets={target_count} radius={radius} moving={moving_percent}% requests/tick={requests_per_tick} fixed_update_ms={maintenance_ms:.3} query_sort_ms={queries_ms:.3} total_ms={:.3}",
                        maintenance_ms + queries_ms,
                    );
                }
            }
        }
    }
}

#[test]
fn tag_filtering_reads_live_components_without_reindexing() {
    let mut app = test_app();
    let enemy = register_tag(&mut app, "Team.Enemy");
    let dead = register_tag(&mut app, "State.Dead");
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let target = spawn_targetable(&mut app, Vec3::X);
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 2.0 },
        TargetingOperation::FilterTags {
            requirements: TagRequirements::new(vec![enemy], vec![dead]).unwrap(),
        },
    ]);
    run_fixed_update(&mut app);
    assert_current_targets(&mut app, source, &definition, &[]);

    add_tag_to_entity(&mut app, target, enemy);
    assert_current_targets(&mut app, source, &definition, &[target]);
    add_tag_to_entity(&mut app, target, dead);
    assert_current_targets(&mut app, source, &definition, &[]);
    remove_tag_from_entity(&mut app, target, dead);
    assert_current_targets(&mut app, source, &definition, &[target]);
    app.world_mut()
        .entity_mut(target)
        .remove::<GameplayTagContainer>();
    assert_current_targets(&mut app, source, &definition, &[]);
}

#[test]
fn targeting_requires_logical_position_and_supports_headless_entities() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let headless = spawn_targetable(&mut app, Vec3::X);
    let visual_only = app
        .world_mut()
        .spawn((Targetable, GlobalTransform::IDENTITY))
        .id();
    let sphere = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);
    run_fixed_update(&mut app);

    assert_current_targets(&mut app, source, &sphere, &[headless]);
    assert!(app.world().get::<Transform>(headless).is_none());
    assert!(app.world().get::<GlobalTransform>(headless).is_none());
    assert_eq!(
        acquire_current(
            &mut app,
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X).with_explicit_target(visual_only),
            targeting_definition(vec![TargetingOperation::SelectExplicitEntity]),
        ),
        Err(TargetingError::TargetNotFound {
            target: visual_only
        })
    );
    assert_eq!(
        acquire_current(
            &mut app,
            visual_only,
            TargetingInput::new(Vec3::ZERO, Vec3::X),
            targeting_definition(vec![TargetingOperation::SelectSelf]),
        ),
        Err(TargetingError::MissingSource {
            source: visual_only
        })
    );
}

#[test]
fn visual_parent_and_child_motion_do_not_change_logical_targeting() {
    let mut app = test_app();
    app.add_plugins(TransformPlugin);
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let parent = app
        .world_mut()
        .spawn(Transform::from_xyz(100.0, 0.0, 0.0))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Targetable,
            LogicalPosition(Vec3::X),
            Transform::from_xyz(1.0, 0.0, 0.0),
            ChildOf(parent),
        ))
        .id();
    let definition = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 2.0 }]);
    run_fixed_update(&mut app);
    app.world_mut().run_schedule(PostUpdate);
    assert_eq!(
        app.world()
            .get::<GlobalTransform>(child)
            .unwrap()
            .translation(),
        Vec3::X * 101.0
    );
    let before = acquire_current(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        Arc::clone(&definition),
    )
    .unwrap();

    app.world_mut()
        .get_mut::<Transform>(parent)
        .unwrap()
        .translation = -Vec3::X * 100.0;
    app.world_mut()
        .get_mut::<Transform>(child)
        .unwrap()
        .translation = Vec3::X * 200.0;
    app.world_mut().run_schedule(PostUpdate);
    run_fixed_update(&mut app);
    let after = acquire_current(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        definition,
    )
    .unwrap();

    assert_eq!(
        app.world()
            .get::<GlobalTransform>(child)
            .unwrap()
            .translation(),
        Vec3::X * 100.0
    );
    assert_eq!(before, after);
    assert_eq!(
        after.get_hits(),
        &[AbilityTargetHit::new(child, Vec3::X, None)]
    );
}

#[test]
fn presentation_interpolation_preserves_target_membership_hit_positions_and_order() {
    let mut app = test_app();
    app.add_plugins(TransformPlugin);
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let first = app
        .world_mut()
        .spawn((
            Targetable,
            LogicalPosition(Vec3::X * 4.0),
            PositionPresentation::interpolated(),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            Targetable,
            LogicalPosition(Vec3::X),
            PositionPresentation::interpolated(),
        ))
        .id();
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 5.0 },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
    ]);
    app.world_mut().run_schedule(PostUpdate);
    app.world_mut().run_schedule(FixedFirst);
    set_target_position(&mut app, first, Vec3::X);
    set_target_position(&mut app, second, Vec3::X * 4.0);
    run_fixed_update(&mut app);
    let expected = acquire_current(
        &mut app,
        source,
        TargetingInput::new(Vec3::ZERO, Vec3::X),
        Arc::clone(&definition),
    )
    .unwrap();
    assert_eq!(
        expected.get_hits(),
        &[
            AbilityTargetHit::new(first, Vec3::X, None),
            AbilityTargetHit::new(second, Vec3::X * 4.0, None),
        ]
    );

    for fraction in [0.0, 0.25, 0.75, 1.0] {
        {
            let mut fixed_time = app.world_mut().resource_mut::<Time<Fixed>>();
            let remainder = fixed_time.overstep();
            let timestep = fixed_time.timestep();
            fixed_time.discard_overstep(remainder);
            fixed_time.accumulate_overstep(timestep.mul_f64(fraction));
        }
        app.world_mut().run_schedule(PostUpdate);
        let actual = acquire_current(
            &mut app,
            source,
            TargetingInput::new(Vec3::ZERO, Vec3::X),
            Arc::clone(&definition),
        )
        .unwrap();
        assert_eq!(actual, expected);
        let visual_first = app.world().get::<Transform>(first).unwrap().translation;
        assert!((visual_first.x - (4.0 - 3.0 * fraction as f32)).abs() < 0.001);
    }
}

#[test]
fn movement_updates_source_and_target_before_independent_request_sampling() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let moving = spawn_targetable(&mut app, Vec3::X * 100.0);
    run_fixed_update(&mut app);
    app.init_resource::<CapturedTargetingResults>();
    app.world_mut().add_observer(
        |event: On<TargetingResultEvent>, mut captured: ResMut<CapturedTargetingResults>| {
            captured.0.push(event.get_result().clone());
        },
    );
    let definition = targeting_definition(vec![TargetingOperation::SelectSphere { radius: 1.5 }]);
    app.add_systems(
        FixedUpdate,
        (move |positions: Query<&LogicalPosition>, mut queue: ResMut<TargetingRequestQueue>| {
            queue.push_request(
                source,
                TargetingInput::new(positions.get(source).unwrap().0, Vec3::X),
                Arc::clone(&definition),
                TargetingContinuation::EmitResult,
            );
        })
        .in_set(GameplayAbilitySystemSet::RequestProducers),
    );
    app.add_systems(
        FixedUpdate,
        (move |mut positions: Query<&mut LogicalPosition>| {
            positions.get_mut(source).unwrap().0 = Vec3::X * 10.0;
            positions.get_mut(moving).unwrap().0 = Vec3::X * 11.0;
        })
        .in_set(GameplayAbilitySystemSet::Movement),
    );

    run_fixed_update(&mut app);

    let captured = app.world().resource::<CapturedTargetingResults>();
    assert_eq!(captured.0.len(), 1);
    let data = captured.0[0].as_ref().unwrap();
    assert_eq!(data.get_origin(), Vec3::X * 10.0);
    assert_eq!(
        data.get_hits(),
        &[AbilityTargetHit::new(moving, Vec3::X * 11.0, None)]
    );
}

#[test]
fn deferred_movement_is_visible_to_ability_tasks_requests_and_targeting_in_one_tick() {
    let mut app = test_app();
    let source = app.world_mut().spawn(LogicalPosition(Vec3::ZERO)).id();
    let moving = spawn_targetable(&mut app, Vec3::X * 100.0);
    run_fixed_update(&mut app);
    app.init_resource::<CapturedTargetingResults>();
    app.world_mut().add_observer(
        |event: On<TargetingResultEvent>, mut captured: ResMut<CapturedTargetingResults>| {
            captured.0.push(event.get_result().clone());
        },
    );
    let definition = targeting_definition(vec![
        TargetingOperation::SelectSphere { radius: 3.0 },
        TargetingOperation::SortByDistance {
            order: TargetingSortOrder::Ascending,
        },
    ]);
    app.add_systems(
        FixedUpdate,
        (move |positions: Query<&LogicalPosition>, mut queue: ResMut<TargetingRequestQueue>| {
            queue.push_request(
                source,
                TargetingInput::new(positions.get(source).unwrap().0, Vec3::X),
                Arc::clone(&definition),
                TargetingContinuation::EmitResult,
            );
        })
        .in_set(GameplayAbilitySystemSet::RequestProducers),
    );
    app.add_systems(
        FixedUpdate,
        (move |positions: Query<&LogicalPosition>| {
            assert_eq!(positions.get(source).unwrap().0, Vec3::X * 20.0);
            assert_eq!(positions.get(moving).unwrap().0, Vec3::X * 21.0);
        })
        .in_set(GameplayAbilitySystemSet::AbilityTasks),
    );
    app.add_systems(
        FixedUpdate,
        (move |mut commands: Commands| {
            commands
                .entity(source)
                .insert(LogicalPosition(Vec3::X * 20.0));
            commands
                .entity(moving)
                .insert(LogicalPosition(Vec3::X * 21.0));
            commands.spawn((Targetable, LogicalPosition(Vec3::X * 22.0)));
        })
        .in_set(GameplayAbilitySystemSet::Movement),
    );

    run_fixed_update(&mut app);

    let captured = app.world().resource::<CapturedTargetingResults>();
    assert_eq!(captured.0.len(), 1);
    let data = captured.0[0].as_ref().unwrap();
    assert_eq!(data.get_origin(), Vec3::X * 20.0);
    assert_eq!(data.primary_entity(), Some(moving));
    assert_eq!(
        data.get_hits()
            .iter()
            .map(AbilityTargetHit::get_position)
            .collect::<Vec<_>>(),
        vec![Vec3::X * 21.0, Vec3::X * 22.0]
    );
}
