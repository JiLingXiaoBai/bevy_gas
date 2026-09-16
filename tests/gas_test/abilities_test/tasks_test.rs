use super::*;
use bevy_gas::{ActiveGameplayAbility, GameplayExecutionOutcome, GameplayExecutionResult};

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
fn ordered_batch_preserves_effect_order_and_stops_at_end() {
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

#[test]
fn startup_effect_finishes_before_the_next_external_activation() {
    let mut app = test_app();
    let ready = register_tag(&mut app, "State.Ready");
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle::default())
        .id();
    let grant_ready = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![ready]),
    ));
    let first =
        Arc::new(
            GameplayAbility::default().with_startup_tasks(vec![AbilityTaskDef::instant(
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                    effect: grant_ready,
                },
            )]),
        );
    let second = Arc::new(
        GameplayAbility::default()
            .with_tags(AbilityTags::default().with_activation_required_tags(vec![ready])),
    );
    let first_handle = give_ability(&mut app, source, first);
    let second_handle = give_ability(&mut app, source, second);
    let ids = {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        [first_handle, second_handle].map(|handle| {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            queue
                .push_activation(source, source, handle, context)
                .unwrap()
        })
    };

    run_gameplay_execution_queue(&mut app);

    let results: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GameplayExecutionResult>>()
        .drain()
        .collect();
    assert_eq!(
        results
            .iter()
            .map(|result| result.request_id)
            .collect::<Vec<_>>(),
        ids
    );
    assert!(
        results
            .iter()
            .all(|result| result.outcome == GameplayExecutionOutcome::Succeeded)
    );
    assert!(active_ability_entity_for_spec(&mut app, second_handle).is_some());
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn nested_startup_activation_finishes_before_parent_batch_continues() {
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
    let child = Arc::new(GameplayAbility::default().with_startup_tasks(vec![
        AbilityTaskDef::instant(AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
            effect: overwrite,
        }),
        AbilityTaskDef::wait_ticks(
            1,
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                effect: instant_add_effect(health, 1000.0),
            },
        ),
        AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
    ]));
    let child_handle = give_ability(&mut app, source, child);
    let parent =
        Arc::new(
            GameplayAbility::default().with_startup_tasks(vec![AbilityTaskDef::instant(
                AbilityTaskOnFinishedDef::Batch {
                    actions: vec![
                        AbilityTaskOnFinishedDef::Batch {
                            actions: vec![AbilityTaskOnFinishedDef::ActivateAbility {
                                handle: child_handle,
                            }],
                        },
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                            effect: instant_add_effect(health, 10.0),
                        },
                    ],
                },
            )]),
        );
    let parent_handle = give_ability(&mut app, source, parent);

    assert!(activate_ability(&mut app, source, target, parent_handle));

    assert_eq!(current_value(&mut app, target, health), 60.0);
    let child_active = active_ability_entity_for_spec(&mut app, child_handle).unwrap();
    let parent_active = active_ability_entity_for_spec(&mut app, parent_handle).unwrap();
    assert_eq!(
        app.world()
            .get::<ActiveGameplayAbility>(child_active)
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Ending
    );
    assert_eq!(
        app.world()
            .get::<ActiveGameplayAbility>(parent_active)
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Active
    );
    assert_eq!(ability_task_count(&mut app), 1);
    run_finished_ability_cleanup(&mut app);
    assert_eq!(ability_task_count(&mut app), 0);
    assert!(active_ability_entity_for_spec(&mut app, child_handle).is_none());
    run_ability_tasks(&mut app);
    run_gameplay_execution_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 60.0);
    assert_eq!(active_ability_count(&mut app), 1);
}

#[derive(Resource, Default)]
struct StartupCancellationObservations {
    removed_abilities: Vec<(AbilitySpecHandle, AbilityActivationStatus)>,
    started_tasks: usize,
}

#[test]
fn child_cancellation_stops_parent_actions_and_preserves_cancelled_status() {
    let mut app = test_app();
    app.init_resource::<StartupCancellationObservations>();
    app.add_observer(
        |remove: On<Remove, ActiveGameplayAbility>,
         abilities: Query<&ActiveGameplayAbility>,
         mut observations: ResMut<StartupCancellationObservations>| {
            let active = abilities.get(remove.entity).unwrap();
            observations
                .removed_abilities
                .push((active.get_spec_handle(), active.get_status()));
        },
    );
    app.add_observer(
        |_: On<Add, AbilityTask>, mut observations: ResMut<StartupCancellationObservations>| {
            observations.started_tasks += 1;
        },
    );
    let parent_tag = register_tag(&mut app, "Ability.Parent");
    let health = register_attribute(&mut app, "Health");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = spawn_attribute_set(&mut app, health, 100.0);
    let child = Arc::new(
        GameplayAbility::default()
            .with_tags(AbilityTags::default().with_cancel_abilities_with_tags(vec![parent_tag])),
    );
    let child_handle = give_ability(&mut app, source, child);
    let parent = Arc::new(
        GameplayAbility::default()
            .with_tags(AbilityTags::default().with_ability_asset_tags(vec![parent_tag]))
            .with_startup_tasks(vec![
                AbilityTaskDef::instant(AbilityTaskOnFinishedDef::Batch {
                    actions: vec![
                        AbilityTaskOnFinishedDef::Batch {
                            actions: vec![AbilityTaskOnFinishedDef::ActivateAbility {
                                handle: child_handle,
                            }],
                        },
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                            effect: instant_add_effect(health, 1000.0),
                        },
                    ],
                }),
                AbilityTaskDef::wait_ticks(
                    1,
                    AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                        effect: instant_add_effect(health, 1000.0),
                    },
                ),
                AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
            ]),
    );
    let parent_handle = give_ability(&mut app, source, parent);

    assert!(activate_ability(&mut app, source, target, parent_handle));

    assert_eq!(current_value(&mut app, target, health), 100.0);
    assert!(active_ability_entity_for_spec(&mut app, parent_handle).is_none());
    assert!(active_ability_entity_for_spec(&mut app, child_handle).is_some());
    assert_eq!(ability_task_count(&mut app), 0);
    let observations = app.world().resource::<StartupCancellationObservations>();
    assert_eq!(observations.started_tasks, 0);
    assert_eq!(
        observations.removed_abilities,
        vec![(parent_handle, AbilityActivationStatus::Cancelled)]
    );
}

#[test]
fn startup_end_prevents_previously_created_wait_tasks_from_executing() {
    for ticks in [0, 1] {
        let mut app = test_app();
        let health = register_attribute(&mut app, "Health");
        let source = app
            .world_mut()
            .spawn(AbilitySystemComponent::default())
            .id();
        let target = spawn_attribute_set(&mut app, health, 100.0);
        let ability = Arc::new(GameplayAbility::default().with_startup_tasks(vec![
            AbilityTaskDef::wait_ticks(
                ticks,
                AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                    effect: instant_add_effect(health, 10.0),
                },
            ),
            AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EndAbility),
        ]));
        let handle = give_ability(&mut app, source, ability);

        assert!(activate_ability(&mut app, source, target, handle));
        assert_eq!(current_value(&mut app, target, health), 100.0);
        run_ability_tasks(&mut app);
        run_gameplay_execution_queue(&mut app);
        assert_eq!(current_value(&mut app, target, health), 100.0);
        run_finished_ability_cleanup(&mut app);
        assert_eq!(ability_task_count(&mut app), 0);
        assert_eq!(active_ability_count(&mut app), 0);
    }
}

#[test]
fn rejected_startup_effect_does_not_stop_later_actions() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = spawn_attribute_set(&mut app, health, 100.0);
    let rejected = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            health,
            ModifierOperation::Add,
            ModifierMagnitude::Flat(1000.0),
        )],
        EffectDurationTicks::Instant,
        None,
        0.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let ability =
        Arc::new(
            GameplayAbility::default().with_startup_tasks(vec![AbilityTaskDef::instant(
                AbilityTaskOnFinishedDef::Batch {
                    actions: vec![
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect: rejected },
                        AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget {
                            effect: instant_add_effect(health, 10.0),
                        },
                        AbilityTaskOnFinishedDef::EndAbility,
                    ],
                },
            )]),
        );
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, target, handle));

    assert_eq!(current_value(&mut app, target, health), 110.0);
    let active = active_ability_entity_for_spec(&mut app, handle).unwrap();
    assert_eq!(
        app.world()
            .get::<ActiveGameplayAbility>(active)
            .unwrap()
            .get_status(),
        AbilityActivationStatus::Ending
    );
}
