use super::*;

#[test]
fn wait_ticks_task_marks_active_ability_ending_after_delay() {
    let mut app = test_app();
    let source = app.world_mut().spawn_empty().id();
    let target = app.world_mut().spawn_empty().id();
    let handle = AbilitySpecHandle::new(7);
    let active_ability = spawn_active_ability(&mut app, source, target, handle);
    let context = AbilityTaskExecutionContext::new(source, handle, 1);
    spawn_ability_task(
        &mut app,
        AbilityTask::wait_ticks(
            active_ability,
            context,
            2,
            AbilityTaskOnFinished::EndAbility,
        ),
    );

    run_ability_tasks(&mut app);
    assert_eq!(
        app.world()
            .entity(active_ability)
            .get::<bevy_gas::ActiveGameplayAbility>()
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Active
    );

    run_ability_tasks(&mut app);
    assert_eq!(
        app.world()
            .entity(active_ability)
            .get::<bevy_gas::ActiveGameplayAbility>()
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Ending
    );
}

#[test]
fn startup_end_ability_stops_later_sibling_tasks() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        vec![
            AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
            AbilityTaskDef::wait_ticks(1, AbilityTaskOnFinishedDef::None),
        ],
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));

    assert_eq!(ability_task_count(&mut app), 0);
    let active_ability = active_ability_entity_for_spec(&mut app, handle).unwrap();
    assert_eq!(
        app.world()
            .entity(active_ability)
            .get::<bevy_gas::ActiveGameplayAbility>()
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Ending
    );
}

#[test]
fn ordered_batch_keeps_effects_queued_and_stops_at_end() {
    for delayed in [false, true] {
        let mut app = test_app();
        let health = register_attribute(&mut app, "Health");
        let source = app
            .world_mut()
            .spawn(AbilitySystemComponent::default())
            .id();
        let target = spawn_attribute_set(&mut app, health, 100.0);
        let overwrite = Arc::new(GameplayEffect::new(
            vec![Modifier::new(
                health,
                ModifierOperation::Override,
                ModifierMagnitude::Flat(50.0),
            )],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        ));
        let batch = AbilityTaskOnFinishedDef::Batch {
            actions: vec![
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect: overwrite },
                AbilityTaskOnFinishedDef::Batch {
                    actions: vec![
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                            effect: instant_add_effect(health, 10.0),
                        },
                        AbilityTaskOnFinishedDef::EndAbility,
                    ],
                },
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                    effect: instant_add_effect(health, 1000.0),
                },
            ],
        };
        let task = if delayed {
            AbilityTaskDef::wait_ticks(1, batch)
        } else {
            AbilityTaskDef::instant(batch)
        };
        let ability = Arc::new(GameplayAbility::default().with_startup_tasks(vec![task]));
        let handle = give_ability(&mut app, source, ability);
        assert!(activate_ability(&mut app, source, target, handle));
        if delayed {
            assert_eq!(current_value(&mut app, target, health), 100.0);
            run_ability_tasks(&mut app);
            assert_eq!(app.world().resource::<GameplayExecutionQueue>().len(), 2);
            run_finished_ability_cleanup(&mut app);
            run_gameplay_execution_queue(&mut app);
        }
        assert_eq!(current_value(&mut app, target, health), 60.0);
        run_finished_ability_cleanup(&mut app);
        assert_eq!(active_ability_count(&mut app), 0);
    }
}

#[test]
fn empty_batch_allows_later_startup_actions() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::default().with_startup_tasks(vec![
        AbilityTaskDef::instant(AbilityTaskOnFinishedDef::Batch {
            actions: Vec::new(),
        }),
        AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
    ]));
    let handle = give_ability(&mut app, source, ability);
    assert!(activate_ability(&mut app, source, source, handle));
    run_finished_ability_cleanup(&mut app);
    assert_eq!(active_ability_count(&mut app), 0);
}
