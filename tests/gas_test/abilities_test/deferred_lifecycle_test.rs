use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy_gas::{AbilitySystemParams, try_activate_ability_by_handle};

fn assert_deferred_asc_discard_cleans_activations(replace: bool) {
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
    let handle = give_ability(&mut app, owner, Arc::clone(&definition));
    let mut replacement = AbilitySystemComponent::default();
    let replacement_handle = replacement.give_ability(definition, 2);
    let mut replacement = Some(replacement);
    let context = AbilityActivationContext::direct(owner, AbilityChainContext::root(handle, 1));

    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            if replace {
                params
                    .commands
                    .entity(owner)
                    .insert(replacement.take().unwrap());
            } else {
                params
                    .commands
                    .entity(owner)
                    .remove::<AbilitySystemComponent>();
            }
            // The queued ASC mutation is not visible to this synchronous activation yet.
            try_activate_ability_by_handle(owner, owner, handle, context.clone(), &mut params)
        })
        .unwrap()
        .unwrap();
    app.world_mut().flush();
    assert_eq!(
        active_ability_count(&mut app),
        0,
        "startup flush must discard activations whose ASC was replaced"
    );
    assert_eq!(ability_task_count(&mut app), 0);
    app.world_mut().run_schedule(FixedUpdate);

    assert_eq!(
        active_ability_count(&mut app),
        0,
        "Cleanup must remove activation entities spawned after their ASC was discarded"
    );
    assert_eq!(
        ability_task_count(&mut app),
        0,
        "Cleanup must remove tasks belonging to discarded activations"
    );
    if replace {
        let spec = app
            .world()
            .get::<AbilitySystemComponent>(owner)
            .unwrap()
            .find_ability_spec(replacement_handle)
            .unwrap();
        assert_eq!(spec.get_level(), 2);
        assert_eq!(spec.get_active_count(), 0);
    }
}

#[test]
fn asc_removal_queued_before_activation_does_not_leave_deferred_instances() {
    assert_deferred_asc_discard_cleans_activations(false);
}

#[test]
fn asc_replacement_queued_before_activation_does_not_leave_deferred_instances() {
    assert_deferred_asc_discard_cleans_activations(true);
}
