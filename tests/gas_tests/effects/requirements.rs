use super::*;

#[test]
fn active_immunity_blocks_matching_incoming_effect() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let source = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));

    assert!(apply_effect(&mut app, target, source, immunity_effect));
    assert!(!apply_effect(&mut app, target, source, incoming));
    assert_eq!(current_value(&mut app, target, health), 100.0);
}

#[test]
fn queued_immunity_is_visible_to_the_next_request() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, immunity_effect, EffectPayload::new(source, None, 1));
        queue.push_application(target, incoming, EffectPayload::new(source, None, 1));
    }

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 100.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn ongoing_tag_requirements_inhibit_and_restore_active_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.Enabled");
    let granted = register_tag(&mut app, "State.Buffed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![granted],
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );

    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );

    add_tag_to_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );
}

#[test]
fn uninhibit_missing_attribute_storage_removes_effect_fail_closed() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.StorageEnabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(active_effect_handles(&app, target).len(), 1);
    app.world_mut().entity_mut(target).remove::<AttributeSet>();
    add_tag_to_entity(&mut app, target, enabled);

    run_effect_tag_requirements_update(&mut app);

    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn queued_granted_tag_converges_ongoing_requirements_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocked = register_tag(&mut app, "State.Blocked");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![blocked]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![blocked]),
    ));

    assert!(apply_effect(&mut app, target, target, buff));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, blocker, EffectPayload::new(target, None, 1));

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn direct_effect_application_converges_requirements_before_returning() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocked = register_tag(&mut app, "State.DirectBlocked");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![blocked]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![blocked]),
    ));

    assert!(apply_effect(&mut app, target, target, buff));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(apply_effect(&mut app, target, target, blocker));
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn requirement_converges_between_two_gameplay_requests() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let disable_immunity = register_tag(&mut app, "State.DisableImmunity");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(Vec::new(), vec![disable_immunity]).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let disable_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![disable_immunity]),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));
    assert!(apply_effect(&mut app, target, source, immunity_effect));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, disable_effect, EffectPayload::new(source, None, 1));
        queue.push_application(target, incoming, EffectPayload::new(source, None, 1));
    }
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, health), 75.0);
}

#[test]
fn queued_tag_source_removal_restores_effect_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocker_asset = register_tag(&mut app, "Effect.Blocker");
    let disabled = register_tag(&mut app, "State.Disabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![blocker_asset], vec![disabled]),
    ));
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![disabled]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            vec![blocker_asset],
        ),
    ));

    assert!(apply_effect(&mut app, target, target, blocker));
    assert!(apply_effect(&mut app, target, target, buff));
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);

    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, cleanse, EffectPayload::new(target, None, 1));
    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 15.0);
}

#[test]
fn queued_tag_triggers_removal_requirement_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let cleanse_tag = register_tag(&mut app, "State.Cleansed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let removable = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::new(vec![cleanse_tag], Vec::new()).unwrap(),
        ),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cleanse_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, removable));
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, cleanse, EffectPayload::new(target, None, 1));
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn source_tag_changes_converge_effect_on_another_target() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "Source.Enabled");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    add_tag_to_entity(&mut app, source, enabled);
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            Vec::new(),
        ),
    ));

    assert!(apply_effect(&mut app, target, source, buff));
    assert_eq!(current_value(&mut app, target, power), 15.0);
    remove_tag_from_entity(&mut app, source, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn queued_source_tag_restores_other_target_effect_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "Source.QueueEnabled");
    let source = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            Vec::new(),
        ),
    ));
    let source_enabler = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![enabled]),
    ));

    assert!(apply_effect(&mut app, target, source, buff));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(source, source_enabler, EffectPayload::new(source, None, 1));

    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 15.0);
}

#[test]
fn duration_tag_cleanup_converges_before_period_execution() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let enabled = register_tag(&mut app, "State.Enabled");
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let enable_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![enabled]),
    ));
    let periodic_damage = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -10.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(1.0), false)),
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, enable_effect));
    assert!(apply_effect(&mut app, target, target, periodic_damage));
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, health), 100.0);
}

#[test]
fn non_converging_requirement_cycle_fails_closed() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let loop_tag = register_tag(&mut app, "State.RequirementLoop");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let looping_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![loop_tag],
            TagRequirements::new(Vec::new(), vec![loop_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, looping_effect));
    run_effect_tag_requirements_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&loop_tag)
    );
}

#[test]
fn mutual_requirement_cycle_is_independent_of_effect_slot_order() {
    let mut app = test_app();
    let first_tag = register_tag(&mut app, "State.FirstLoop");
    let second_tag = register_tag(&mut app, "State.SecondLoop");
    let first_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![first_tag],
            TagRequirements::new(Vec::new(), vec![second_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let second_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![second_tag],
            TagRequirements::new(Vec::new(), vec![first_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let first_target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let second_target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();

    assert!(apply_effect(
        &mut app,
        first_target,
        first_target,
        first_effect.clone()
    ));
    assert!(apply_effect(
        &mut app,
        first_target,
        first_target,
        second_effect.clone()
    ));
    assert!(apply_effect(
        &mut app,
        second_target,
        second_target,
        second_effect
    ));
    assert!(apply_effect(
        &mut app,
        second_target,
        second_target,
        first_effect
    ));

    run_effect_tag_requirements_update(&mut app);
    assert!(active_effect_handles(&app, first_target).is_empty());
    assert!(active_effect_handles(&app, second_target).is_empty());
}

#[test]
fn removal_tag_requirement_cleans_up_active_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let cleanse = register_tag(&mut app, "State.Cleansed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::new(vec![cleanse], Vec::new()).unwrap(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 15.0);

    add_tag_to_entity(&mut app, target, cleanse);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}
