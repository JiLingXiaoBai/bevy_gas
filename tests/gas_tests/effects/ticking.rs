use super::*;

#[test]
fn periodic_effect_executes_on_application_and_each_period() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 2.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(2.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 12.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 12.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 14.0);
}

#[test]
fn tag_only_periodic_effect_does_not_require_attribute_set() {
    let mut app = test_app();
    let active_tag = register_tag(&mut app, "State.Periodic");
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(1.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![active_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    run_effect_period_tick(&mut app);

    assert_eq!(active_effect_handles(&app, target).len(), 1);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .is_some_and(|tags| tags.has_tag(&active_tag))
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn zero_tick_period_behaves_like_duration_modifier_without_period_ticks() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 15.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
}

#[test]
fn zero_tick_period_restores_modifier_after_inhibition() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.Enabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    add_tag_to_entity(&mut app, target, enabled);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 15.0);

    remove_tag_from_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);

    add_tag_to_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 15.0);
}
