use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy_gas::{
    AbilityActivationContext, AbilityActivationReason, AbilityInputBindingError,
    AbilityInputBindings, AbilitySpecHandle, AbilitySystemComponent, ActiveGameplayAbility,
    GameplayAbility, GameplayAbilitySystemBundle, GameplayAbilitySystemPlugin,
    GameplayAbilitySystemSet, GameplayExecutionQueue,
};
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Primary,
    Secondary,
    Utility,
}

#[test]
fn rebinding_and_removal_preserve_other_actions_and_iteration_order() {
    let first = AbilitySpecHandle::new(1);
    let second = AbilitySpecHandle::new(2);
    let mut bindings = AbilityInputBindings::<Action>::default();

    assert!(bindings.is_empty());
    assert_eq!(bindings.bind(Action::Primary, first), None);
    assert_eq!(bindings.bind(Action::Secondary, second), None);
    assert_eq!(bindings.bind(Action::Utility, first), None);
    assert_eq!(bindings.bind(Action::Primary, second), Some(first));
    assert_eq!(bindings.len(), 3);
    assert_eq!(
        bindings
            .iter()
            .map(|(&action, handle)| (action, handle))
            .collect::<Vec<_>>(),
        vec![
            (Action::Primary, second),
            (Action::Secondary, second),
            (Action::Utility, first)
        ],
    );

    assert_eq!(bindings.unbind(&Action::Secondary), Some(second));
    assert_eq!(bindings.unbind(&Action::Secondary), None);
    assert_eq!(bindings.get(&Action::Primary), Some(second));
    assert_eq!(bindings.get(&Action::Utility), Some(first));
    assert_eq!(bindings.bind(Action::Secondary, first), None);
    assert_eq!(
        bindings
            .iter()
            .map(|(&action, handle)| (action, handle))
            .collect::<Vec<_>>(),
        vec![
            (Action::Primary, second),
            (Action::Utility, first),
            (Action::Secondary, first)
        ],
    );

    assert_eq!(bindings.unbind_ability(first), 2);
    assert_eq!(bindings.unbind_ability(first), 0);
    assert_eq!(
        bindings.iter().collect::<Vec<_>>(),
        vec![(&Action::Primary, second)]
    );
    bindings.clear();
    assert!(bindings.is_empty());
}

#[test]
fn resolution_distinguishes_unbound_actions_from_revoked_abilities() {
    let mut asc = AbilitySystemComponent::default();
    let ability = asc.give_ability(Arc::new(GameplayAbility::default()), 1);
    let mut bindings = AbilityInputBindings::<Action>::default();

    assert_eq!(
        bindings.resolve(&Action::Primary, &asc),
        Err(AbilityInputBindingError::UnboundInput),
    );
    bindings.bind(Action::Primary, ability);
    bindings.bind(Action::Secondary, ability);
    assert_eq!(bindings.resolve(&Action::Primary, &asc), Ok(ability));

    assert!(asc.clear_ability(ability));
    assert_eq!(
        bindings.resolve(&Action::Primary, &asc),
        Err(AbilityInputBindingError::AbilityNotGranted { handle: ability }),
    );
    assert_eq!(bindings.unbind_ability(ability), 2);
    assert_eq!(
        bindings.resolve(&Action::Secondary, &asc),
        Err(AbilityInputBindingError::UnboundInput),
    );
}

#[test]
fn each_actor_can_bind_one_shared_definition_to_a_different_action() {
    let mut world = World::new();
    let ability = Arc::new(GameplayAbility::default());
    let mut first_asc = AbilitySystemComponent::default();
    let first_handle = first_asc.give_ability(ability.clone(), 1);
    let mut first_bindings = AbilityInputBindings::<Action>::default();
    first_bindings.bind(Action::Primary, first_handle);
    let first_actor = world.spawn((first_asc, first_bindings)).id();

    let mut second_asc = AbilitySystemComponent::default();
    let second_handle = second_asc.give_ability(ability.clone(), 5);
    let mut second_bindings = AbilityInputBindings::<Action>::default();
    second_bindings.bind(Action::Secondary, second_handle);
    let second_actor = world.spawn((second_asc, second_bindings)).id();

    let mut query = world.query::<(&AbilitySystemComponent, &AbilityInputBindings<Action>)>();
    let (first_asc, first_bindings) = query.get(&world, first_actor).unwrap();
    let (second_asc, second_bindings) = query.get(&world, second_actor).unwrap();
    assert_eq!(
        first_bindings.resolve(&Action::Primary, first_asc),
        Ok(first_handle)
    );
    assert_eq!(first_bindings.get(&Action::Secondary), None);
    assert_eq!(
        second_bindings.resolve(&Action::Secondary, second_asc),
        Ok(second_handle)
    );
    assert_eq!(second_bindings.get(&Action::Primary), None);
    let first_spec = first_asc.find_ability_spec(first_handle).unwrap();
    let second_spec = second_asc.find_ability_spec(second_handle).unwrap();
    assert!(Arc::ptr_eq(
        first_spec.get_ability(),
        second_spec.get_ability()
    ));
    assert_eq!(first_spec.get_level(), 1);
    assert_eq!(second_spec.get_level(), 5);
}

#[derive(Resource)]
struct ControlledActor(Entity);

#[derive(Resource, Default)]
struct PendingInputActivations(VecDeque<(Entity, AbilitySpecHandle)>);

fn capture_primary_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    actor: Res<ControlledActor>,
    actors: Query<(&AbilitySystemComponent, &AbilityInputBindings<Action>)>,
    mut pending: ResMut<PendingInputActivations>,
) {
    if !keyboard.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let (asc, bindings) = actors.get(actor.0).unwrap();
    let handle = bindings.resolve(&Action::Primary, asc).unwrap();
    pending.0.push_back((actor.0, handle));
}

fn submit_buffered_input(
    mut pending: ResMut<PendingInputActivations>,
    mut queue: ResMut<GameplayExecutionQueue>,
) {
    while let Some((source, handle)) = pending.0.pop_front() {
        let context = AbilityActivationContext::input(source, queue.new_root_chain(handle));
        queue.push_activation(source, source, handle, context);
    }
}

#[test]
fn buffered_input_survives_frames_without_ticks_and_is_consumed_once() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayAbilitySystemPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<PendingInputActivations>()
        .add_systems(PreUpdate, capture_primary_input.after(InputSystems))
        .add_systems(
            FixedUpdate,
            submit_buffered_input.in_set(GameplayAbilitySystemSet::RequestProducers),
        );

    let mut actor = GameplayAbilitySystemBundle::default();
    let handle = actor.ability_system.give_ability(
        Arc::new(GameplayAbility::default().with_allow_multiple_instances(true)),
        1,
    );
    let mut bindings = AbilityInputBindings::<Action>::default();
    bindings.bind(Action::Primary, handle);
    let source = app.world_mut().spawn((actor, bindings)).id();
    app.insert_resource(ControlledActor(source));

    // Drive frame input and fixed ticks independently, without wall-clock timing.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyQ);
    app.world_mut().run_schedule(PreUpdate);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    for _ in 0..3 {
        app.world_mut().run_schedule(PreUpdate);
    }
    assert_eq!(app.world().resource::<PendingInputActivations>().0.len(), 1);

    // A second press before the next tick must also survive the frame boundary.
    {
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.release(KeyCode::KeyQ);
        keyboard.press(KeyCode::KeyQ);
    }
    app.world_mut().run_schedule(PreUpdate);
    assert_eq!(app.world().resource::<PendingInputActivations>().0.len(), 2);
    app.world_mut().run_schedule(FixedUpdate);
    assert!(
        app.world()
            .resource::<PendingInputActivations>()
            .0
            .is_empty()
    );
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());

    let mut active = app.world_mut().query::<&ActiveGameplayAbility>();
    assert_eq!(active.iter(app.world()).count(), 2);
    for ability in active.iter(app.world()) {
        assert_eq!(ability.get_source(), source);
        assert_eq!(ability.get_spec_handle(), handle);
        assert_eq!(ability.get_activation_context().get_instigator(), source);
        assert_eq!(
            ability.get_activation_context().get_reason(),
            AbilityActivationReason::Input
        );
        assert_eq!(ability.get_chain().unwrap().get_visited(), &[handle]);
    }

    // Additional catch-up ticks must not re-read this rendered frame's key edge.
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(active.iter(app.world()).count(), 2);
}
