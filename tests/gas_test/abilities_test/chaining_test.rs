use super::*;

#[test]
fn chained_ability_activation_blocks_cycles() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let first_handle = AbilitySpecHandle::new(0);
    let second_handle = AbilitySpecHandle::new(1);
    let first = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: second_handle,
            },
        )],
        None,
        None,
        Vec::new(),
        false,
        true,
    ));
    let second = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: first_handle,
            },
        )],
        None,
        None,
        Vec::new(),
        false,
        true,
    ));

    assert_eq!(give_ability(&mut app, source, first), first_handle);
    assert_eq!(give_ability(&mut app, source, second), second_handle);

    assert!(activate_ability(&mut app, source, source, first_handle));
    assert!(
        app.world()
            .resource::<bevy_tools::GameplayExecutionQueue>()
            .is_empty()
    );
    assert_eq!(active_ability_count(&mut app), 2);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(first_handle)
            .unwrap()
            .get_active_count(),
        1
    );
}

#[test]
fn chained_startup_activation_can_cancel_deferred_parent() {
    let mut app = test_app();
    let parent_tag = register_tag(&mut app, "Ability.Parent");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let parent_handle = AbilitySpecHandle::new(0);
    let child_handle = AbilitySpecHandle::new(1);
    let parent = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![parent_tag],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: child_handle,
            },
        )],
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let child = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            vec![parent_tag],
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

    assert_eq!(give_ability(&mut app, source, parent), parent_handle);
    assert_eq!(give_ability(&mut app, source, child), child_handle);
    assert!(activate_ability(&mut app, source, source, parent_handle));

    assert_eq!(active_ability_count(&mut app), 1);
    let asc = app
        .world()
        .entity(source)
        .get::<AbilitySystemComponent>()
        .unwrap();
    assert_eq!(
        asc.find_ability_spec(parent_handle)
            .unwrap()
            .get_active_count(),
        0
    );
    assert_eq!(
        asc.find_ability_spec(child_handle)
            .unwrap()
            .get_active_count(),
        1
    );
}

#[test]
fn chained_activation_inherits_context_and_activation_effects_use_payload() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let damage = register_attribute(&mut app, "Damage");
    let instigator = app.world_mut().spawn_empty().id();
    let causer = app.world_mut().spawn_empty().id();
    let attributes = attribute_set(&app, power, 7.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let target = spawn_attribute_set(&mut app, damage, 0.0);
    let manager = app
        .world()
        .resource::<bevy_tools::AttributeIdManager>()
        .clone();
    let source_snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<bevy_tools::AttributeSet>()
        .unwrap()
        .make_snapshot(source);
    let first_handle = AbilitySpecHandle::new(0);
    let second_handle = AbilitySpecHandle::new(1);
    let first = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ActivateAbility {
                handle: second_handle,
            },
        )],
        None,
        None,
        Vec::new(),
        false,
        true,
    ));
    let activation_effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            damage,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(ContextPayloadMagnitude {
                expected_instigator: instigator,
                expected_causer: causer,
                snapshot_attribute: power,
            })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let second = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![activation_effect],
        false,
        true,
    ));

    assert_eq!(give_ability(&mut app, source, first), first_handle);
    assert_eq!(give_ability(&mut app, source, second), second_handle);

    let context =
        AbilityActivationContext::direct(source, AbilityChainContext::root(first_handle, 0))
            .with_instigator(instigator)
            .with_causer(Some(causer))
            .with_source_snapshot(source_snapshot);
    activate_ability_with_context(&mut app, source, target.into(), first_handle, context).unwrap();
    let first_active = active_ability_entity_for_spec(&mut app, first_handle).unwrap();

    run_ability_tasks(&mut app);
    run_gameplay_execution_queue(&mut app);

    assert_eq!(current_value(&mut app, target, damage), 7.0);
    let second_context = active_ability_context_for_spec(&mut app, second_handle).unwrap();
    assert_eq!(second_context.get_instigator(), instigator);
    assert_eq!(second_context.get_causer(), Some(causer));
    assert_eq!(
        second_context
            .get_source_snapshot()
            .unwrap()
            .get_current_value(&manager, power),
        Ok(Some(7.0))
    );
    assert_eq!(
        second_context.get_reason(),
        AbilityActivationReason::Chained {
            parent_ability: first_active,
        }
    );
    assert_eq!(second_context.get_chain().unwrap().get_depth(), 1);
    assert_eq!(
        second_context.get_chain().unwrap().get_visited(),
        &[first_handle, second_handle]
    );
}

#[test]
fn ability_chain_context_rejects_depth_beyond_limit() {
    let mut chain = AbilityChainContext::root(AbilitySpecHandle::new(0), 42);
    for index in 1..=AbilityChainContext::MAX_DEPTH {
        chain = chain.next(AbilitySpecHandle::new(index as u32)).unwrap();
    }

    let err = chain.next(AbilitySpecHandle::new(99)).unwrap_err();

    assert_eq!(
        err,
        AbilityChainError::DepthExceeded {
            chain_id: 42,
            max_depth: AbilityChainContext::MAX_DEPTH,
        }
    );
}
