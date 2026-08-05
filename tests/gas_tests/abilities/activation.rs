use super::*;

#[test]
fn ability_disallows_multiple_instances_by_default() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
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

    assert!(activate_ability(&mut app, source, source, handle));
    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(active_ability_count(&mut app), 1);
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

#[test]
fn ability_allows_multiple_instances_when_enabled() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        true,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(active_ability_count(&mut app), 2);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        2
    );
}

#[test]
fn ability_activation_required_and_blocked_tags_are_enforced() {
    let mut app = test_app();
    let required = register_tag(&mut app, "State.Weapon.Ready");
    let blocked = register_tag(&mut app, "State.Silenced");
    let source = app
        .world_mut()
        .spawn((
            AbilitySystemComponent::default(),
            GameplayTagContainer::default(),
        ))
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![required],
            vec![blocked],
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        true,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(!activate_ability(&mut app, source, source, handle));

    add_tag_to_entity(&mut app, source, required);
    assert!(activate_ability(&mut app, source, source, handle));

    add_tag_to_entity(&mut app, source, blocked);
    assert!(!activate_ability(&mut app, source, source, handle));
}

#[test]
fn active_ability_block_tags_prevent_matching_ability_activation() {
    let mut app = test_app();
    let channel_tag = register_tag(&mut app, "Ability.Channel");
    let movement_tag = register_tag(&mut app, "Ability.Movement");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let channel = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![channel_tag],
            Vec::new(),
            vec![movement_tag],
            Vec::new(),
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let movement = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![movement_tag],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let channel_handle = give_ability(&mut app, source, channel);
    let movement_handle = give_ability(&mut app, source, movement);

    assert!(activate_ability(&mut app, source, source, channel_handle));
    assert!(!activate_ability(&mut app, source, source, movement_handle));
}

#[test]
fn try_activate_ability_by_handle_returns_error_for_missing_spec() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let missing_handle = AbilitySpecHandle::new(999);

    let err = activate_ability_result(&mut app, source, source, missing_handle).unwrap_err();

    assert_eq!(
        err,
        AbilityActivationError::AbilityNotFound {
            source,
            handle: missing_handle
        }
    );
}
