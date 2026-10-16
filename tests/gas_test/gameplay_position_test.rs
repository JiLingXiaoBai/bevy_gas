use crate::support_test::run_fixed_update;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_gas::{
    GameplayAbilitySystemPlugin, GameplayAbilitySystemRuntimePlugin, GameplayTagPlugin,
    LogicalPosition, PositionPresentation, PositionPresentationError, PositionPresentationPlugin,
    RandomPlugin, UniqueNamePlugin,
};

fn presentation_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PositionPresentationPlugin));
    app.init_schedule(FixedUpdate);
    app
}

fn render_at(app: &mut App, fraction: f64) {
    {
        let mut fixed_time = app.world_mut().resource_mut::<Time<Fixed>>();
        let remainder = fixed_time.overstep();
        let timestep = fixed_time.timestep();
        fixed_time.discard_overstep(remainder);
        fixed_time.accumulate_overstep(timestep.mul_f64(fraction));
    }
    app.world_mut().run_schedule(PostUpdate);
}

fn move_in_fixed_tick(app: &mut App, entity: Entity, position: Vec3) {
    app.world_mut().run_schedule(FixedFirst);
    app.world_mut()
        .get_mut::<LogicalPosition>(entity)
        .unwrap()
        .0 = position;
    run_fixed_update(app);
}

fn translation(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<Transform>(entity).unwrap().translation
}

fn presentation_error(app: &App, entity: Entity) -> Option<PositionPresentationError> {
    app.world()
        .get::<PositionPresentation>(entity)
        .unwrap()
        .last_error()
}

fn assert_position(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 0.0001,
        "actual={actual:?}, expected={expected:?}"
    );
}

#[test]
fn presentation_is_optional_and_initial_spawn_uses_current_logical_position() {
    let mut app = presentation_app();
    let headless = app.world_mut().spawn(LogicalPosition(Vec3::X * 3.0)).id();
    let presented = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::X * 7.0),
            PositionPresentation::default(),
        ))
        .id();
    assert!(app.world().get::<Transform>(presented).is_some());

    render_at(&mut app, 0.25);
    assert_position(translation(&app, presented), Vec3::X * 7.0);
    assert_eq!(presentation_error(&app, presented), None);
    app.world_mut().run_schedule(FixedFirst);
    run_fixed_update(&mut app);
    render_at(&mut app, 0.75);

    assert!(app.world().get::<Transform>(headless).is_none());
    assert!(app.world().get::<GlobalTransform>(headless).is_none());
    assert_eq!(
        app.world().get::<LogicalPosition>(headless).unwrap().0,
        Vec3::X * 3.0
    );
}

#[test]
fn interpolation_uses_the_final_fixed_pair_across_zero_one_and_multiple_ticks() {
    let mut app = presentation_app();
    let entity = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::ZERO),
            PositionPresentation::interpolated(),
        ))
        .id();
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::ZERO);

    move_in_fixed_tick(&mut app, entity, Vec3::X * 10.0);
    render_at(&mut app, 0.25);
    assert_position(translation(&app, entity), Vec3::X * 2.5);
    render_at(&mut app, 0.75);
    assert_position(translation(&app, entity), Vec3::X * 7.5);

    move_in_fixed_tick(&mut app, entity, Vec3::X * 20.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 30.0);
    for (fraction, expected_x) in [(0.0, 20.0), (0.5, 25.0), (1.0, 30.0)] {
        render_at(&mut app, fraction);
        assert_position(translation(&app, entity), Vec3::X * expected_x);
    }
    assert_eq!(
        app.world().get::<LogicalPosition>(entity).unwrap().0,
        Vec3::X * 30.0
    );
}

#[test]
fn direct_mode_and_runtime_mode_changes_control_only_presentation() {
    let mut app = presentation_app();
    let entity = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::X * 10.0),
            PositionPresentation::direct(),
        ))
        .id();
    render_at(&mut app, 0.0);
    assert_position(translation(&app, entity), Vec3::X * 10.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 20.0);
    render_at(&mut app, 0.25);
    assert_position(translation(&app, entity), Vec3::X * 20.0);

    app.world_mut()
        .get_mut::<PositionPresentation>(entity)
        .unwrap()
        .set_interpolation_enabled(true);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 30.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 25.0);
    app.world_mut()
        .get_mut::<PositionPresentation>(entity)
        .unwrap()
        .set_interpolation_enabled(false);
    render_at(&mut app, 0.0);
    assert_position(translation(&app, entity), Vec3::X * 30.0);
    assert_eq!(
        app.world().get::<LogicalPosition>(entity).unwrap().0,
        Vec3::X * 30.0
    );
}

#[test]
fn teleport_reset_discards_old_history_then_resumes_interpolation() {
    let mut app = presentation_app();
    let entity = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::ZERO),
            PositionPresentation::interpolated(),
        ))
        .id();
    render_at(&mut app, 0.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 10.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 5.0);

    app.world_mut()
        .get_mut::<LogicalPosition>(entity)
        .unwrap()
        .0 = Vec3::X * 100.0;
    app.world_mut()
        .get_mut::<PositionPresentation>(entity)
        .unwrap()
        .reset();
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 100.0);
    render_at(&mut app, 0.75);
    assert_position(translation(&app, entity), Vec3::X * 100.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 110.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 105.0);
}

#[test]
fn presentation_preserves_rotation_and_scale_and_propagates_visual_children() {
    let mut app = presentation_app();
    let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let scale = Vec3::splat(2.0);
    let root = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::new(10.0, 2.0, 0.0)),
            PositionPresentation::direct(),
            Transform {
                translation: Vec3::splat(-1.0),
                rotation,
                scale,
            },
        ))
        .id();
    let visual_child = app
        .world_mut()
        .spawn((Transform::from_translation(Vec3::X), ChildOf(root)))
        .id();
    render_at(&mut app, 0.5);

    let transform = app.world().get::<Transform>(root).unwrap();
    assert_eq!(transform.rotation, rotation);
    assert_eq!(transform.scale, scale);
    assert_position(transform.translation, Vec3::new(10.0, 2.0, 0.0));
    assert_position(
        app.world()
            .get::<GlobalTransform>(visual_child)
            .unwrap()
            .translation(),
        Vec3::new(10.0, 4.0, 0.0),
    );

    move_in_fixed_tick(&mut app, root, Vec3::new(20.0, 2.0, 0.0));
    render_at(&mut app, 0.25);
    assert_position(
        app.world()
            .get::<GlobalTransform>(visual_child)
            .unwrap()
            .translation(),
        Vec3::new(20.0, 4.0, 0.0),
    );
    assert_eq!(
        app.world().get::<Transform>(root).unwrap().rotation,
        rotation
    );
    assert_eq!(app.world().get::<Transform>(root).unwrap().scale, scale);
}

#[test]
fn missing_components_report_errors_without_overwriting_visual_state_and_recover() {
    let mut app = presentation_app();
    let missing_logical = app
        .world_mut()
        .spawn((
            PositionPresentation::default(),
            Transform::from_translation(Vec3::X * 3.0),
        ))
        .id();
    let missing_transform = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::Y * 4.0),
            PositionPresentation::default(),
        ))
        .id();
    app.world_mut()
        .entity_mut(missing_transform)
        .remove::<Transform>();
    render_at(&mut app, 0.5);

    assert_eq!(
        presentation_error(&app, missing_logical),
        Some(PositionPresentationError::MissingLogicalPosition)
    );
    assert_position(translation(&app, missing_logical), Vec3::X * 3.0);
    assert_eq!(
        presentation_error(&app, missing_transform),
        Some(PositionPresentationError::MissingTransform)
    );
    assert!(app.world().get::<Transform>(missing_transform).is_none());
    app.world_mut()
        .entity_mut(missing_logical)
        .insert(LogicalPosition(Vec3::X * 5.0));
    app.world_mut()
        .entity_mut(missing_transform)
        .insert(Transform::default());
    render_at(&mut app, 0.5);

    assert_eq!(presentation_error(&app, missing_logical), None);
    assert_eq!(presentation_error(&app, missing_transform), None);
    assert_position(translation(&app, missing_logical), Vec3::X * 5.0);
    assert_position(translation(&app, missing_transform), Vec3::Y * 4.0);
}

#[test]
fn invalid_logical_positions_clear_history_and_finite_recovery_is_immediate() {
    let mut app = presentation_app();
    let entity = app
        .world_mut()
        .spawn((LogicalPosition(Vec3::ZERO), PositionPresentation::default()))
        .id();
    render_at(&mut app, 0.0);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let before = translation(&app, entity);
        move_in_fixed_tick(&mut app, entity, Vec3::new(invalid, 0.0, 0.0));
        render_at(&mut app, 0.5);
        assert_eq!(
            presentation_error(&app, entity),
            Some(PositionPresentationError::NonFiniteLogicalPosition)
        );
        assert_eq!(translation(&app, entity), before);
        move_in_fixed_tick(&mut app, entity, Vec3::X * 8.0);
        render_at(&mut app, 0.25);
        assert_eq!(presentation_error(&app, entity), None);
        assert_position(translation(&app, entity), Vec3::X * 8.0);
    }
}

#[test]
fn parented_presentation_reports_an_error_until_the_entity_becomes_a_root() {
    let mut app = presentation_app();
    let parent = app
        .world_mut()
        .spawn(Transform::from_translation(Vec3::X * 10.0))
        .id();
    let child = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::X * 50.0),
            PositionPresentation::direct(),
            Transform::from_translation(Vec3::X),
            ChildOf(parent),
        ))
        .id();
    render_at(&mut app, 0.5);

    assert_eq!(
        presentation_error(&app, child),
        Some(PositionPresentationError::ParentedEntity)
    );
    assert_position(translation(&app, child), Vec3::X);
    assert_position(
        app.world()
            .get::<GlobalTransform>(child)
            .unwrap()
            .translation(),
        Vec3::X * 11.0,
    );
    assert_eq!(
        app.world().get::<LogicalPosition>(child).unwrap().0,
        Vec3::X * 50.0
    );
    app.world_mut().entity_mut(child).remove::<ChildOf>();
    render_at(&mut app, 0.5);

    assert_eq!(presentation_error(&app, child), None);
    assert_position(translation(&app, child), Vec3::X * 50.0);
}

#[test]
fn logical_component_replacement_and_readdition_discard_stale_interpolation_history() {
    let mut app = presentation_app();
    let entity = app
        .world_mut()
        .spawn((
            LogicalPosition(Vec3::ZERO),
            PositionPresentation::interpolated(),
        ))
        .id();
    render_at(&mut app, 0.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 10.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 5.0);

    app.world_mut()
        .entity_mut(entity)
        .insert(LogicalPosition(Vec3::X * 100.0));
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 100.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 110.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 105.0);

    app.world_mut()
        .entity_mut(entity)
        .remove::<LogicalPosition>()
        .insert(LogicalPosition(Vec3::X * 200.0));
    app.world_mut().clear_trackers();
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 200.0);
    move_in_fixed_tick(&mut app, entity, Vec3::X * 210.0);
    render_at(&mut app, 0.5);
    assert_position(translation(&app, entity), Vec3::X * 205.0);
    assert_eq!(presentation_error(&app, entity), None);
}

#[test]
fn default_and_configured_gas_plugin_groups_enable_position_presentation() {
    for configured in [false, true] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        if configured {
            app.add_plugins(GameplayAbilitySystemPlugin::with_additional_costs::<()>());
        } else {
            app.add_plugins(GameplayAbilitySystemPlugin);
        }
        let entity = app
            .world_mut()
            .spawn((
                LogicalPosition(Vec3::X * 8.0),
                PositionPresentation::interpolated(),
            ))
            .id();

        render_at(&mut app, 0.0);
        assert_position(translation(&app, entity), Vec3::X * 8.0);
        move_in_fixed_tick(&mut app, entity, Vec3::X * 12.0);
        render_at(&mut app, 0.5);
        assert_position(translation(&app, entity), Vec3::X * 10.0);
    }
}

#[test]
fn disabled_presentation_and_runtime_only_leave_visual_transforms_untouched() {
    for installation in 0..3 {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.init_schedule(FixedFirst);
        match installation {
            0 => {
                app.add_plugins(
                    GameplayAbilitySystemPlugin
                        .build()
                        .disable::<PositionPresentationPlugin>(),
                );
            }
            1 => {
                app.add_plugins(
                    GameplayAbilitySystemPlugin::with_additional_costs::<()>()
                        .build()
                        .disable::<PositionPresentationPlugin>(),
                );
            }
            _ => {
                app.add_plugins((
                    UniqueNamePlugin,
                    GameplayTagPlugin,
                    RandomPlugin,
                    GameplayAbilitySystemRuntimePlugin,
                ));
            }
        }
        let entity = app
            .world_mut()
            .spawn((
                LogicalPosition(Vec3::X * 8.0),
                PositionPresentation::interpolated(),
                Transform::from_translation(Vec3::Y * 3.0),
            ))
            .id();

        app.world_mut().run_schedule(FixedFirst);
        app.world_mut()
            .get_mut::<LogicalPosition>(entity)
            .unwrap()
            .0 = Vec3::X * 12.0;
        run_fixed_update(&mut app);
        render_at(&mut app, 0.5);

        assert_position(translation(&app, entity), Vec3::Y * 3.0);
        assert_eq!(
            app.world().get::<LogicalPosition>(entity).unwrap().0,
            Vec3::X * 12.0
        );
    }
}
