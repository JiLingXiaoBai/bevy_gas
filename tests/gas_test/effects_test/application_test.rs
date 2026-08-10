use super::*;

use bevy::prelude::{Entity, Res, ResMut, Resource};

#[derive(Resource)]
struct EffectUnderTest(Arc<GameplayEffect>);

#[derive(Resource)]
struct TargetUnderTest(Entity);

#[derive(Resource, Default)]
struct ApplyResult(bool);

fn apply_effect_system(
    effect: Res<EffectUnderTest>,
    target: Res<TargetUnderTest>,
    mut params: AbilitySystemParams,
    mut result: ResMut<ApplyResult>,
) {
    let payload = EffectPayload::new(target.0, None, 1);
    result.0 = apply_gameplay_effect(target.0, &effect.0, &mut params, &payload).is_ok();
}

#[test]
fn tag_only_active_effect_does_not_require_attribute_set() {
    let mut app = test_app();
    app.init_resource::<ApplyResult>();
    let tag = register_tag(&mut app, "State.Stunned");
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![tag]),
    ));

    app.insert_resource(TargetUnderTest(target));
    app.insert_resource(EffectUnderTest(effect));
    app.world_mut()
        .run_system_once(apply_effect_system)
        .unwrap();

    assert!(app.world().resource::<ApplyResult>().0);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&tag)
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn failed_effect_application_does_not_leave_duration_modifier() {
    let mut app = test_app();
    app.init_resource::<ApplyResult>();
    let tag = register_tag(&mut app, "State.Buffed");
    let health = register_attribute(&mut app, "Health");
    let manager = app
        .world()
        .resource::<bevy_tools::AttributeIdManager>()
        .clone();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, health, 10.0, None)
        .unwrap();
    let target = app.world_mut().spawn(attributes).id();
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            health,
            ModifierOperation::Add,
            ModifierMagnitude::Flat(5.0),
        )],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![tag]),
    ));

    app.insert_resource(TargetUnderTest(target));
    app.insert_resource(EffectUnderTest(effect));
    app.world_mut()
        .run_system_once(apply_effect_system)
        .unwrap();

    assert!(!app.world().resource::<ApplyResult>().0);
    let mut attributes = app.world_mut().entity_mut(target);
    let mut attributes = attributes.get_mut::<AttributeSet>().unwrap();
    assert_eq!(
        attributes.get_current_value(&manager, health),
        Ok(Some(10.0))
    );
}

#[test]
fn probability_zero_blocks_application_and_one_allows_it() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let blocked = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        0.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let allowed = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(!apply_effect(&mut app, target, target, blocked));
    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert!(apply_effect(&mut app, target, target, allowed));
    assert_eq!(current_value(&mut app, target, health), 20.0);
}

#[test]
fn invalid_probability_returns_a_specific_application_error() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        f32::NAN,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(matches!(
        apply_effect_result(&mut app, target, target, effect),
        Err(GameplayEffectApplicationError::InvalidProbability { probability })
            if probability.is_nan()
    ));
}

#[test]
fn execution_preflight_preserves_effects_when_target_state_changed() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let removable = register_tag(&mut app, "Effect.Removable");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let existing = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![removable], Vec::new()),
    ));
    assert!(apply_effect(&mut app, target, target, existing));
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(Vec::new(), Vec::new()).with_remove_effects_with_tags(vec![removable]),
    ));
    let payload = EffectPayload::new(target, None, 1);
    let plan = app
        .world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            prepare_gameplay_effect(target, &effect, &mut params, &payload)
        })
        .unwrap()
        .unwrap();

    app.world_mut().entity_mut(target).remove::<AttributeSet>();
    let mut plan = Some(plan);
    let result = app
        .world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            execute_gameplay_effect_plan(plan.take().unwrap(), &mut params)
        })
        .unwrap();

    assert_eq!(
        result,
        Err(GameplayEffectApplicationError::MissingAttributeSet { target })
    );
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn non_positive_or_nan_duration_ticks_reject_application() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);

    for duration in [0.0, -1.0, f32::NAN] {
        let effect = Arc::new(GameplayEffect::new(
            vec![add_modifier(health, 10.0)],
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(duration)),
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        ));
        assert!(!apply_effect(&mut app, target, target, effect));
    }

    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn tag_granting_effect_without_tag_container_rolls_back_modifiers() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let granted = register_tag(&mut app, "State.Buffed");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted]),
    ));

    assert!(!apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}
