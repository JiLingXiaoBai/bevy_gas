use super::*;

#[test]
fn activating_ability_cancels_matching_active_abilities() {
    let mut app = test_app();
    let stance_tag = register_tag(&mut app, "Ability.Stance");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let stance = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![stance_tag],
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
    let breaker = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            vec![stance_tag],
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
    let stance_handle = give_ability(&mut app, source, stance);
    let breaker_handle = give_ability(&mut app, source, breaker);

    assert!(activate_ability(&mut app, source, source, stance_handle));
    assert_eq!(active_ability_count(&mut app), 1);
    assert!(activate_ability(&mut app, source, source, breaker_handle));
    assert_eq!(active_ability_count(&mut app), 1);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(stance_handle)
            .unwrap()
            .get_active_count(),
        0
    );
}

#[test]
fn repeated_same_batch_cancellation_cleans_live_ability_only_once() {
    let mut app = test_app();
    let victim_tag = register_tag(&mut app, "Victim.Ability");
    let survivor_tag = register_tag(&mut app, "Survivor.Ability");
    let shared_block_tag = register_tag(&mut app, "Block.SharedAbility");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();

    let victim = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![victim_tag],
            Vec::new(),
            vec![shared_block_tag],
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
    let survivor = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![survivor_tag],
            Vec::new(),
            vec![shared_block_tag],
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
    let canceller = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            vec![victim_tag],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        true,
    ));
    let blocked = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![shared_block_tag],
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
        true,
    ));

    let victim_handle = give_ability(&mut app, source, victim);
    let survivor_handle = give_ability(&mut app, source, survivor);
    let canceller_handle = give_ability(&mut app, source, canceller);
    let blocked_handle = give_ability(&mut app, source, blocked);
    assert!(activate_ability(&mut app, source, source, victim_handle));
    assert!(activate_ability(&mut app, source, source, survivor_handle));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        for handle in [canceller_handle, canceller_handle, blocked_handle] {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            queue.push_activation(source, source, handle, context);
        }
    }
    run_gameplay_execution_queue(&mut app);

    let asc = app
        .world()
        .entity(source)
        .get::<AbilitySystemComponent>()
        .unwrap();
    assert_eq!(
        asc.find_ability_spec(victim_handle)
            .unwrap()
            .get_active_count(),
        0
    );
    assert_eq!(
        asc.find_ability_spec(survivor_handle)
            .unwrap()
            .get_active_count(),
        1
    );
    assert_eq!(
        asc.find_ability_spec(blocked_handle)
            .unwrap()
            .get_active_count(),
        0
    );
    assert!(asc.get_blocked_ability_tags().has_tag(&shared_block_tag));
}

#[test]
fn failed_activation_does_not_cancel_matching_active_abilities() {
    let mut app = test_app();
    let stance_tag = register_tag(&mut app, "Ability.Stance");
    let required_tag = register_tag(&mut app, "State.BreakerReady");
    let source = app
        .world_mut()
        .spawn((
            AbilitySystemComponent::default(),
            GameplayTagContainer::default(),
        ))
        .id();
    let stance = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![stance_tag],
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
    let breaker = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            vec![stance_tag],
            Vec::new(),
            vec![required_tag],
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let stance_handle = give_ability(&mut app, source, stance);
    let breaker_handle = give_ability(&mut app, source, breaker);

    assert!(activate_ability(&mut app, source, source, stance_handle));
    assert!(!activate_ability(&mut app, source, source, breaker_handle));
    assert_eq!(active_ability_count(&mut app), 1);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(stance_handle)
            .unwrap()
            .get_active_count(),
        1
    );
}

#[test]
fn cleanup_finished_ability_despawns_startup_tasks_and_is_repeatable() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::wait_ticks(
            10,
            AbilityTaskOnFinishedDef::None,
        )],
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(active_ability_count(&mut app), 1);
    assert_eq!(ability_task_count(&mut app), 1);

    run_finished_ability_cleanup(&mut app);
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(ability_task_count(&mut app), 0);
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

    run_finished_ability_cleanup(&mut app);
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(ability_task_count(&mut app), 0);
}

#[test]
fn ability_spec_preserves_input_id_and_clear_rebuilds_indices() {
    let mut asc = AbilitySystemComponent::default();
    let first = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let second = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));

    let first_handle = asc.give_ability(first, 2, Some(4));
    let second_handle = asc.give_ability(second, 3, Some(8));

    assert_eq!(
        asc.find_ability_spec(first_handle).unwrap().get_input_id(),
        Some(4)
    );
    assert_eq!(
        asc.find_ability_spec(second_handle).unwrap().get_input_id(),
        Some(8)
    );
    assert!(asc.clear_ability(first_handle));
    assert!(asc.find_ability_spec(first_handle).is_none());
    assert_eq!(asc.find_ability_spec(second_handle).unwrap().get_level(), 3);
    assert_eq!(
        asc.find_ability_spec(second_handle).unwrap().get_input_id(),
        Some(8)
    );
}

#[test]
fn ability_spec_tracks_input_pressed_state() {
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let mut spec = GameplayAbilitySpec::new(AbilitySpecHandle::new(0), ability, 1, Some(4));

    assert!(!spec.is_input_pressed());

    spec.set_input_pressed(true);
    assert!(spec.is_input_pressed());

    spec.set_input_pressed(false);
    assert!(!spec.is_input_pressed());
}

#[test]
fn clear_ability_returns_false_while_spec_is_active() {
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
    assert!(
        !app.world_mut()
            .entity_mut(source)
            .get_mut::<AbilitySystemComponent>()
            .unwrap()
            .clear_ability(handle)
    );
    assert!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .is_some()
    );
}
