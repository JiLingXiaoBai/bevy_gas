//! Runs logical slot bindings and buffered input without a window or wall-clock delays.
//!
//! Run with `cargo run --example ability_input_bindings`. Keyboard edges are simulated here;
//! a windowed application uses the same capture system after Bevy's `InputSystems`.

use bevy::ecs::system::RunSystemOnce;
use bevy::input::InputSystems;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_tools::prelude::*;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Arc;

type ExampleResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Slot(u8),
}

#[derive(Resource)]
struct ControlledActor(Entity);

// Store the resolved handle so changing slots does not retarget an already captured press.
#[derive(Resource, Default)]
struct BufferedActivations(VecDeque<(Entity, AbilitySpecHandle)>);

fn main() -> ExampleResult<()> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        LogPlugin::default(),
        GameplayAbilitySystemPlugin,
    ))
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<BufferedActivations>()
    .add_systems(PreUpdate, capture_input.after(InputSystems))
    .add_systems(
        FixedUpdate,
        submit_buffered_activations.in_set(GameplayAbilitySystemSet::RequestProducers),
    );
    app.finish();
    app.cleanup();

    let definition = Arc::new(GameplayAbility::default().with_allow_multiple_instances(true));
    let mut actor = GameplayAbilitySystemBundle::default();
    let first = actor.ability_system.give_ability(definition.clone(), 1);
    let second = actor.ability_system.give_ability(definition, 2);
    let mut bindings = AbilityInputBindings::default();
    bindings.bind(Action::Slot(0), first);
    bindings.bind(Action::Slot(1), second);
    let source = app.world_mut().spawn((actor, bindings)).id();
    app.insert_resource(ControlledActor(source));

    // Capture one press, then simulate frames that do not run a fixed tick.
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

    // Rebind slot zero while the old activation is buffered. It still targets `first`.
    app.world_mut()
        .get_mut::<AbilityInputBindings<Action>>(source)
        .ok_or("the controlled actor has no input bindings")?
        .bind(Action::Slot(0), second);
    app.world_mut().run_schedule(FixedUpdate);
    info!("After the buffered press: first ability has one active instance, second has zero");
    report(&mut app)?;

    // Catch-up ticks consume no additional presses, even though Q remains held.
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedUpdate);
    info!("After two catch-up ticks: instance counts are unchanged");
    report(&mut app)?;

    // A fresh press resolves the new slot binding and activates `second`.
    {
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.release(KeyCode::KeyQ);
        keyboard.press(KeyCode::KeyQ);
    }
    app.world_mut().run_schedule(PreUpdate);
    app.world_mut().run_schedule(FixedUpdate);
    info!("After a new press: both abilities have one active instance");
    report(&mut app)?;
    Ok(())
}

fn capture_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    actor: Res<ControlledActor>,
    actors: Query<(&AbilitySystemComponent, &AbilityInputBindings<Action>)>,
    mut buffered: ResMut<BufferedActivations>,
) {
    let Ok((ability_system, bindings)) = actors.get(actor.0) else {
        return;
    };

    // Explicit device-to-action order also defines precedence for simultaneous key presses.
    // A gamepad or UI adapter can resolve the same actions and append to the same FIFO.
    for (key, action) in [
        (KeyCode::KeyQ, Action::Slot(0)),
        (KeyCode::KeyE, Action::Slot(1)),
    ] {
        if !keyboard.just_pressed(key) {
            continue;
        }
        match bindings.resolve(&action, ability_system) {
            Ok(handle) => buffered.0.push_back((actor.0, handle)),
            Err(error) => warn!(?action, %error, "Unable to resolve ability input"),
        }
    }
}

fn submit_buffered_activations(
    mut buffered: ResMut<BufferedActivations>,
    mut queue: ResMut<GameplayExecutionQueue>,
) {
    while let Some((source, handle)) = buffered.0.pop_front() {
        let context = AbilityActivationContext::input(source, queue.new_root_chain(handle));
        // This example self-targets. A game can capture a target with the input, or submit
        // a targeting request and let its continuation enqueue the ability activation.
        queue.push_activation(source, source, handle, context);
    }
}

fn report(app: &mut App) -> ExampleResult<()> {
    app.world_mut()
        .run_system_once(report_abilities)
        .map_err(|error| format!("ability reporting system failed: {error:?}"))??;
    Ok(())
}

fn report_abilities(
    actor: Res<ControlledActor>,
    abilities: Query<&AbilitySystemComponent>,
) -> ExampleResult<()> {
    for spec in abilities.get(actor.0)?.get_ability_specs() {
        info!(
            handle = spec.get_handle().get_value(),
            level = spec.get_level(),
            active_count = spec.get_active_count(),
            "Granted ability"
        );
    }
    Ok(())
}
