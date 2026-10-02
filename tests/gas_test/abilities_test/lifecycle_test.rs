use super::*;
use bevy_gas::ActiveGameplayAbility;

#[test]
fn active_ability_lookups_distinguish_owners_with_the_same_spec_handle() {
    let mut app = test_app();
    let first_owner = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let second_owner = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let definition = Arc::new(GameplayAbility::default());
    let first_handle = give_ability(&mut app, first_owner, Arc::clone(&definition));
    let second_handle = give_ability(&mut app, second_owner, definition);
    assert_eq!(first_handle, second_handle);

    let first_context =
        AbilityActivationContext::direct(first_owner, AbilityChainContext::root(first_handle, 11));
    activate_ability_with_context(
        &mut app,
        first_owner,
        first_owner.into(),
        first_handle,
        first_context,
    )
    .unwrap();
    assert!(active_ability_entity_for_spec(&mut app, second_owner, second_handle).is_none());
    assert!(active_ability_context_for_spec(&mut app, second_owner, second_handle).is_none());

    let second_context =
        AbilityActivationContext::input(second_owner, AbilityChainContext::root(second_handle, 22));
    activate_ability_with_context(
        &mut app,
        second_owner,
        second_owner.into(),
        second_handle,
        second_context,
    )
    .unwrap();

    let first_active = active_ability_entity_for_spec(&mut app, first_owner, first_handle).unwrap();
    let second_active =
        active_ability_entity_for_spec(&mut app, second_owner, second_handle).unwrap();
    assert_ne!(first_active, second_active);
    for (owner, handle, active, chain_id, reason) in [
        (
            first_owner,
            first_handle,
            first_active,
            11,
            AbilityActivationReason::Direct,
        ),
        (
            second_owner,
            second_handle,
            second_active,
            22,
            AbilityActivationReason::Input,
        ),
    ] {
        let ability = app.world().get::<ActiveGameplayAbility>(active).unwrap();
        assert_eq!(ability.get_source(), owner);
        assert_eq!(ability.get_spec_handle(), handle);
        let context = active_ability_context_for_spec(&mut app, owner, handle).unwrap();
        assert_eq!(context.get_instigator(), owner);
        assert_eq!(context.get_chain().unwrap().get_chain_id(), chain_id);
        assert_eq!(context.get_reason(), reason);
    }

    app.world_mut().despawn(first_active);
    app.world_mut().flush();
    assert!(active_ability_entity_for_spec(&mut app, first_owner, first_handle).is_none());
    assert!(active_ability_context_for_spec(&mut app, first_owner, first_handle).is_none());
    assert_eq!(
        active_ability_entity_for_spec(&mut app, second_owner, second_handle),
        Some(second_active)
    );
    let second_context =
        active_ability_context_for_spec(&mut app, second_owner, second_handle).unwrap();
    assert_eq!(second_context.get_instigator(), second_owner);
    assert_eq!(second_context.get_chain().unwrap().get_chain_id(), 22);
}

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
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::EndAbility,
        )],
        None,
        None,
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
            queue
                .push_activation(source, source, handle, context)
                .unwrap();
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
        vec![
            AbilityTaskDef::wait_ticks(10, AbilityTaskOnFinishedDef::None),
            AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
        ],
        None,
        None,
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
fn ability_spec_preserves_granted_level_and_clear_rebuilds_indices() {
    let mut asc = AbilitySystemComponent::default();
    let first = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::EndAbility,
        )],
        None,
        None,
        false,
    ));
    let second = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::EndAbility,
        )],
        None,
        None,
        false,
    ));

    let first_handle = asc.give_ability(first, 2).unwrap();
    let second_handle = asc.give_ability(second, 3).unwrap();

    assert_eq!(asc.find_ability_spec(first_handle).unwrap().get_level(), 2);
    assert_eq!(asc.find_ability_spec(second_handle).unwrap().get_level(), 3);
    assert!(asc.clear_ability(first_handle));
    assert!(asc.find_ability_spec(first_handle).is_none());
    assert_eq!(asc.find_ability_spec(second_handle).unwrap().get_level(), 3);
    assert_eq!(
        asc.find_ability_spec(second_handle).unwrap().get_handle(),
        second_handle
    );
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

#[test]
fn removing_active_components_releases_each_shared_block_only_once() {
    let mut app = test_app();
    let shared = register_tag(&mut app, "Block.Shared");
    let owner = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let definition = Arc::new(GameplayAbility::new(
        AbilityTags::new(Vec::new(), Vec::new(), vec![shared], Vec::new(), Vec::new()),
        Vec::new(),
        None,
        None,
        true,
    ));
    let first = give_ability(&mut app, owner, Arc::clone(&definition));
    let second = give_ability(&mut app, owner, definition);
    assert!(activate_ability(&mut app, owner, owner, first));
    assert!(activate_ability(&mut app, owner, owner, second));
    let first_entity = active_ability_entity_for_spec(&mut app, owner, first).unwrap();
    let second_entity = active_ability_entity_for_spec(&mut app, owner, second).unwrap();

    app.world_mut()
        .entity_mut(first_entity)
        .remove::<ActiveGameplayAbility>();
    app.world_mut().flush();
    run_finished_ability_cleanup(&mut app);
    let asc = app.world().get::<AbilitySystemComponent>(owner).unwrap();
    assert_eq!(asc.find_ability_spec(first).unwrap().get_active_count(), 0);
    assert_eq!(asc.find_ability_spec(second).unwrap().get_active_count(), 1);
    assert!(asc.get_blocked_ability_tags().has_tag(&shared));

    app.world_mut().despawn(second_entity);
    app.world_mut().flush();
    let asc = app.world().get::<AbilitySystemComponent>(owner).unwrap();
    assert_eq!(asc.find_ability_spec(second).unwrap().get_active_count(), 0);
    assert!(!asc.get_blocked_ability_tags().has_tag(&shared));
    assert!(activate_ability(&mut app, owner, owner, first));
}

#[test]
fn replacing_an_active_component_terminates_its_old_activation_and_tasks() {
    let mut app = test_app();
    let owner = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let definition = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::wait_ticks(
            10,
            AbilityTaskOnFinishedDef::None,
        )],
        None,
        None,
        false,
    ));
    let handle = give_ability(&mut app, owner, definition);
    assert!(activate_ability(&mut app, owner, owner, handle));
    let entity = active_ability_entity_for_spec(&mut app, owner, handle).unwrap();
    let replacement = app
        .world()
        .get::<ActiveGameplayAbility>(entity)
        .unwrap()
        .clone();
    app.world_mut().entity_mut(entity).insert(replacement);
    app.world_mut().flush();
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(ability_task_count(&mut app), 0);
    assert_eq!(
        app.world()
            .get::<AbilitySystemComponent>(owner)
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        0
    );
    assert!(activate_ability(&mut app, owner, owner, handle));
}

#[test]
fn replacing_an_asc_terminates_its_old_instances_without_touching_new_grants() {
    let mut app = test_app();
    let owner = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let definition = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::wait_ticks(
            10,
            AbilityTaskOnFinishedDef::None,
        )],
        None,
        None,
        false,
    ));
    let old_handle = give_ability(&mut app, owner, Arc::clone(&definition));
    assert!(activate_ability(&mut app, owner, owner, old_handle));
    let mut replacement = AbilitySystemComponent::default();
    let new_handle = replacement.give_ability(definition, 2).unwrap();
    app.world_mut().entity_mut(owner).insert(replacement);
    app.world_mut().flush();
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(ability_task_count(&mut app), 0);
    let spec = app
        .world()
        .get::<AbilitySystemComponent>(owner)
        .unwrap()
        .find_ability_spec(new_handle)
        .unwrap();
    assert_eq!(spec.get_level(), 2);
    assert_eq!(spec.get_active_count(), 0);
    assert!(activate_ability(&mut app, owner, owner, new_handle));
}
