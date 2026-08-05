use crate::common_test::{
    activate_ability_with_context, add_modifier, add_tag_to_entity, attribute_set, current_value,
    empty_effect_tags, give_ability, register_attribute, register_tag, run_fixed_update, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_tools::{
    AbilityActivationContext, AbilitySystemComponent, AbilityTags, AbilityTargetData,
    AbilityTargetHit, AbilityTaskDef, AbilityTaskOnFinishedDef, EffectDurationTicks,
    GameplayAbility, GameplayEffect, GameplayTagContainer, StackingPolicy, TagRequirements,
    Targetable, TargetingCandidateQuery, TargetingContinuation, TargetingDefinition,
    TargetingDefinitionError, TargetingError, TargetingInput, TargetingOperation,
    TargetingRequestQueue, TargetingSortOrder, acquire_targets,
};
use std::sync::Arc;

fn targeting_definition(operations: Vec<TargetingOperation>) -> Arc<TargetingDefinition> {
    Arc::new(TargetingDefinition::new(operations).unwrap())
}

fn assert_approx_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= f32::EPSILON);
}

fn spawn_targetable(app: &mut App, position: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            Targetable,
            GlobalTransform::from_translation(position),
            GameplayTagContainer::default(),
        ))
        .id()
}

fn acquire(
    app: &mut App,
    source: Entity,
    input: TargetingInput,
    definition: Arc<TargetingDefinition>,
) -> Result<AbilityTargetData, TargetingError> {
    app.world_mut()
        .run_system_once(move |query: TargetingCandidateQuery| {
            acquire_targets(source, input, &definition, &query)
        })
        .unwrap()
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
    let source = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
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
    let source = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
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
    let source = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
    let unmarked = app
        .world_mut()
        .spawn(GlobalTransform::from_translation(Vec3::X))
        .id();
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
    let source = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
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
        .spawn((AbilitySystemComponent::default(), GlobalTransform::IDENTITY))
        .id();
    let target = spawn_targetable(&mut app, Vec3::X);
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let definition = targeting_definition(vec![TargetingOperation::SelectExplicitEntity]);

    let context = {
        let mut queue = app
            .world_mut()
            .resource_mut::<bevy_tools::GameplayExecutionQueue>();
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

    let target_data = app
        .world_mut()
        .run_system_once(move |query: Query<&bevy_tools::ActiveGameplayAbility>| {
            query
                .iter()
                .find(|active| active.get_spec_handle() == handle)
                .and_then(|active| active.get_activation_context().get_target_data())
                .cloned()
        })
        .unwrap()
        .unwrap();
    assert_eq!(target_data.primary_entity(), Some(target));
}

#[test]
fn targeting_queue_processes_entire_batch() {
    const REQUEST_COUNT: usize = 33;

    let mut app = test_app();
    let source = app.world_mut().spawn(GlobalTransform::IDENTITY).id();
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
        .spawn((AbilitySystemComponent::default(), GlobalTransform::IDENTITY))
        .id();
    let first_attributes = attribute_set(&app, health, 100.0);
    let first = app
        .world_mut()
        .spawn((
            Targetable,
            GlobalTransform::from_translation(Vec3::X),
            first_attributes,
        ))
        .id();
    let second_attributes = attribute_set(&app, health, 100.0);
    let second = app
        .world_mut()
        .spawn((
            Targetable,
            GlobalTransform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
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
        Vec::new(),
        false,
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
            .resource_mut::<bevy_tools::GameplayExecutionQueue>();
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
fn activation_effects_apply_to_every_entity_in_target_data() {
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
        Vec::new(),
        None,
        None,
        vec![damage],
        true,
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
        AbilityActivationContext::direct(source, bevy_tools::AbilityChainContext::root(handle, 1))
            .with_target_data(target_data);

    activate_ability_with_context(&mut app, source, first, handle, context).unwrap();

    assert_approx_eq(current_value(&mut app, first, health), 90.0);
    assert_approx_eq(current_value(&mut app, second, health), 90.0);
}
