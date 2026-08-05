use super::*;

#[test]
fn ability_activation_commits_cost_and_cooldown_then_cooldown_blocks_reactivation() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let cooldown_tag = register_tag(&mut app, "Cooldown.Fireball");
    let attributes = attribute_set(&app, mana, 50.0);
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let cost = instant_add_effect(mana, -20.0);
    let cooldown = Arc::new(bevy_tools::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        Some(cooldown),
        Some(cost),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 30.0);
    assert!(
        app.world()
            .entity(source)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&cooldown_tag)
    );

    run_finished_ability_cleanup(&mut app);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        0
    );
    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 30.0);
}

#[test]
fn ability_cost_fails_when_it_would_drop_attribute_below_zero() {
    let mut app = test_app();
    let stamina = register_attribute(&mut app, "Stamina");
    let attributes = attribute_set(&app, stamina, 10.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        Some(instant_add_effect(stamina, -20.0)),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, stamina), 10.0);
}

#[test]
fn cooldown_prepare_failure_does_not_spend_ability_cost() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let cooldown_tag = register_tag(&mut app, "Cooldown.NoContainer");
    let attributes = attribute_set(&app, mana, 50.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let cooldown = Arc::new(bevy_tools::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        Some(cooldown),
        Some(instant_add_effect(mana, -20.0)),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 50.0);
    assert_eq!(active_ability_count(&mut app), 0);
}

#[test]
fn activation_effect_failure_does_not_block_ability_success() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "Effect.MissingTargetContainer");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let best_effort_effect = Arc::new(bevy_tools::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![best_effort_effect],
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        1
    );
}
