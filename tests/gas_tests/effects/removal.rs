use super::*;

#[test]
fn remove_effects_with_tags_cleans_existing_effect_before_new_application() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let buff_tag = register_tag(&mut app, "Effect.Buff.Power");
    let target = spawn_attribute_set(&mut app, damage, 10.0);
    let old_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(damage, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![buff_tag], Vec::new()),
    ));
    let replacing_effect_tags = EffectTags::new(
        Vec::new(),
        Vec::new(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        Vec::new(),
        vec![buff_tag],
    );
    let replacing_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(damage, 1.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        replacing_effect_tags,
    ));

    assert!(apply_effect(&mut app, target, target, old_effect));
    assert_eq!(current_value(&mut app, target, damage), 20.0);

    assert!(apply_effect(&mut app, target, target, replacing_effect));
    assert_eq!(current_value(&mut app, target, damage), 11.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn tag_only_instant_cleanse_does_not_require_attribute_set() {
    let mut app = test_app();
    let effect_tag = register_tag(&mut app, "Effect.TagOnly");
    let granted_tag = register_tag(&mut app, "State.TagOnly");
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let active_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![effect_tag], vec![granted_tag]),
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
            vec![effect_tag],
        ),
    ));

    assert!(apply_effect(&mut app, target, target, active_effect));
    assert!(apply_effect(&mut app, target, target, cleanse));

    assert!(active_effect_handles(&app, target).is_empty());
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .is_some_and(|tags| !tags.has_tag(&granted_tag))
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn queued_effect_can_remove_effect_created_earlier_in_same_batch() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let buff_tag = register_tag(&mut app, "Effect.Buff.Power");
    let granted_tag = register_tag(&mut app, "State.Buffed");
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
        effect_tags(vec![buff_tag], vec![granted_tag]),
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
            vec![buff_tag],
        ),
    ));
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, buff, EffectPayload::new(target, None, 1));
        queue.push_application(target, cleanse, EffectPayload::new(target, None, 1));
    }

    run_fixed_update(&mut app);
    assert!(active_effect_handles(&app, target).is_empty());
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted_tag)
    );
}
