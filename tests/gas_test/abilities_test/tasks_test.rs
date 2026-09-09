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
