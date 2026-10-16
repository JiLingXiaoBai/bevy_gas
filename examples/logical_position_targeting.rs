//! Separates fixed-tick targeting from interpolated presentation without opening a window.
//!
//! Run with `cargo run --example logical_position_targeting`. Frames advance by 25 ms,
//! while gameplay advances every 100 ms. Targeting always uses the current logical position.

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformSystems;
use bevy_gas::prelude::*;
use bevy_gas::{
    Targetable, TargetingContinuation, TargetingInput, TargetingOperation, TargetingResultEvent,
};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

type ExampleResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Component)]
struct Caster;

#[derive(Resource)]
struct SearchDefinition(Arc<TargetingDefinition>);

fn main() -> ExampleResult<()> {
    let definition =
        TargetingDefinition::new(vec![TargetingOperation::SelectSphere { radius: 3.0 }])?;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        LogPlugin::default(),
        TransformPlugin,
        GameplayAbilitySystemPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_duration(Duration::from_millis(100)))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        25,
    )))
    .insert_resource(SearchDefinition(Arc::new(definition)))
    .add_systems(
        FixedUpdate,
        (move_targets, submit_search)
            .chain()
            .in_set(GameplayAbilitySystemSet::RequestProducers),
    )
    .add_systems(
        PostUpdate,
        report_positions.after(TransformSystems::Propagate),
    )
    .add_observer(report_targeting);

    // A server-only actor needs no Transform or GlobalTransform.
    app.world_mut().spawn((Caster, LogicalPosition(Vec3::ZERO)));
    // Presentation is explicit and only drives the root's visual translation.
    app.world_mut().spawn((
        Targetable,
        LogicalPosition(Vec3::new(2.0, 0.0, 0.0)),
        PositionPresentation::interpolated(),
    ));
    app.finish();
    app.cleanup();
    for _ in 0..13 {
        app.update();
    }
    Ok(())
}

fn move_targets(mut targets: Query<&mut LogicalPosition, With<Targetable>>) {
    for mut position in &mut targets {
        position.0.x += 1.0;
    }
}

fn submit_search(
    casters: Query<(Entity, &LogicalPosition), With<Caster>>,
    definition: Res<SearchDefinition>,
    mut requests: ResMut<TargetingRequestQueue>,
) {
    for (source, position) in &casters {
        requests.push_request(
            source,
            TargetingInput::new(position.0, Vec3::X),
            Arc::clone(&definition.0),
            TargetingContinuation::EmitResult,
        );
    }
}

fn report_targeting(event: On<TargetingResultEvent>) {
    match event.get_result() {
        Ok(targets) => info!("Fixed tick acquired logical hits: {:?}", targets.get_hits()),
        Err(error) => info!("Fixed tick targeting: {error}"),
    }
}

fn report_positions(targets: Query<(&LogicalPosition, &GlobalTransform), With<Targetable>>) {
    for (position, transform) in &targets {
        info!(
            logical_x = position.0.x,
            rendered_x = transform.translation().x,
            "Frame position"
        );
    }
}
