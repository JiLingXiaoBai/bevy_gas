use super::*;

#[test]
fn linear_stacking_respects_stack_limit() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 2),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert_eq!(current_value(&mut app, target, power), 15.0);
    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(!apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 20.0);

    let handles = active_effect_handles(&app, target);
    assert_eq!(handles.len(), 1);
    let active_effect = app
        .world()
        .entity(target)
        .get::<ActiveGameplayEffects>()
        .and_then(|effects| effects.get(handles[0]))
        .unwrap();
    assert_eq!(active_effect.get_stack_count(), 2);
}

#[test]
fn tag_only_stack_and_single_stack_expiration_do_not_require_attribute_set() {
    let mut app = test_app();
    let stacked_tag = register_tag(&mut app, "State.Stacked");
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        2,
        StackMagnitudePolicy::None,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::KeepCurrentTick,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        stacking,
        effect_tags(Vec::new(), vec![stacked_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    let handle = active_effect_handles(&app, target)[0];
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .map(|effect| effect.get_stack_count()),
        Some(2)
    );

    run_effect_duration_tick(&mut app);

    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .map(|effect| effect.get_stack_count()),
        Some(1)
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn queued_applications_stack_against_same_batch_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ));
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, effect.clone(), EffectPayload::new(target, None, 1));
        queue.push_application(target, effect, EffectPayload::new(target, None, 1));
    }

    run_fixed_update(&mut app);
    let handles = active_effect_handles(&app, target);
    assert_eq!(handles.len(), 1);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handles[0]))
            .unwrap()
            .get_stack_count(),
        2
    );
}

#[test]
fn remove_single_stack_expiration_decrements_stack_before_removal() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        3,
        StackMagnitudePolicy::Linear,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::ResetOnSuccessfulStack,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, armor), 20.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    let handle = active_effect_handles(&app, target)[0];
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .unwrap()
            .get_stack_count(),
        1
    );

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn zero_tick_period_updates_linear_stack_magnitude() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn zero_tick_period_updates_remove_single_stack_expiration() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        3,
        StackMagnitudePolicy::Linear,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::ResetOnSuccessfulStack,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, armor), 20.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn overflow_refresh_duration_extends_existing_stack_lifetime() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        1,
        StackMagnitudePolicy::None,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::KeepCurrentTick,
        StackOverflowPolicy::RefreshDuration,
        StackExpirationPolicy::RemoveAllStacks,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
        None,
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    run_effect_duration_tick(&mut app);
    assert!(apply_effect(&mut app, target, target, effect));

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn aggregate_by_source_keeps_separate_stacks_per_source() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first_source = app.world_mut().spawn_empty().id();
    let second_source = app.world_mut().spawn_empty().id();
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateBySource, 3),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, first_source, effect.clone()));
    assert!(apply_effect(&mut app, target, first_source, effect.clone()));
    assert!(apply_effect(&mut app, target, second_source, effect));

    assert_eq!(active_effect_handles(&app, target).len(), 2);
    assert_eq!(current_value(&mut app, target, power), 25.0);
}
