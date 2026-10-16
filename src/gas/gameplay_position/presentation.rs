use super::{LogicalPosition, PositionPresentationError};
use bevy::prelude::{
    ChildOf, Component, Discard, Fixed, Has, On, Query, Res, Time, Transform, Vec3,
};

/// Opts a root entity into one-way synchronization from logical position to rendering.
///
/// This component requires a `Transform`, but the entity must provide its own
/// [`LogicalPosition`]. Only translation is written; rotation and scale remain unchanged.
/// Attach meshes and other visual offsets as children of this root. Entities with a
/// `ChildOf` relationship are rejected because logical positions are world coordinates.
///
/// Interpolation is enabled by default and renders between the previous and current fixed
/// positions, introducing one fixed tick of presentation latency. Call [`Self::reset`]
/// after teleportation or other discontinuous changes to prevent interpolation across them.
#[derive(Component, Debug)]
#[require(Transform)]
pub struct PositionPresentation {
    previous: Option<Vec3>,
    interpolate: bool,
    last_error: Option<PositionPresentationError>,
}

impl Default for PositionPresentation {
    fn default() -> Self {
        Self::interpolated()
    }
}

impl PositionPresentation {
    /// Creates presentation that directly copies the current logical position each frame.
    pub const fn direct() -> Self {
        Self {
            previous: None,
            interpolate: false,
            last_error: None,
        }
    }

    /// Creates presentation that interpolates between consecutive fixed-tick positions.
    ///
    /// Until history is captured, the current position is copied directly.
    pub const fn interpolated() -> Self {
        Self {
            previous: None,
            interpolate: true,
            last_error: None,
        }
    }

    /// Returns whether this entity uses fixed-tick position interpolation.
    pub const fn is_interpolation_enabled(&self) -> bool {
        self.interpolate
    }

    /// Enables or disables interpolation according to `enabled`.
    ///
    /// Changing modes discards history so the next presentation uses the current position.
    pub fn set_interpolation_enabled(&mut self, enabled: bool) {
        if self.interpolate != enabled {
            self.interpolate = enabled;
            self.reset();
        }
    }

    /// Discards position history so the next presentation uses the current logical position.
    ///
    /// Call this alongside a teleport or a correction outside normal fixed-tick movement.
    /// A subsequent fixed tick captures a new starting position and resumes interpolation.
    /// This does not change the logical position, transform, or last reported error.
    pub fn reset(&mut self) {
        self.previous = None;
    }

    /// Returns the most recent capture or synchronization error, if any.
    ///
    /// A successful capture or synchronization clears the error.
    pub const fn last_error(&self) -> Option<PositionPresentationError> {
        self.last_error
    }

    fn record_error(&mut self, error: PositionPresentationError) {
        self.previous = None;
        self.last_error = Some(error);
    }
}

/// Prevents interpolation across discarded or replaced logical-position components.
pub(crate) fn reset_discarded_logical_position_presentation(
    event: On<Discard<LogicalPosition>>,
    mut presentations: Query<&mut PositionPresentation>,
) {
    if let Ok(mut presentation) = presentations.get_mut(event.entity) {
        presentation.reset();
    }
}
type PositionHistoryQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static LogicalPosition>,
        Has<Transform>,
        Has<ChildOf>,
        &'static mut PositionPresentation,
    ),
>;

type PositionPresentationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static LogicalPosition>,
        Option<&'static mut Transform>,
        Has<ChildOf>,
        &'static mut PositionPresentation,
    ),
>;

/// Captures opted-in entities' positions before fixed-tick gameplay mutates them.
///
/// Schedule this in `FixedFirst`. Every fixed tick replaces the previous position, so a
/// render frame with several fixed ticks interpolates only the final pair of positions.
/// Invalid presentation configurations record an error and clear their history.
pub fn capture_logical_position_history_system(mut query: PositionHistoryQuery) {
    for (logical, has_transform, has_parent, mut presentation) in &mut query {
        match validate_position(logical, has_transform, has_parent) {
            Ok(position) => {
                presentation.previous = Some(position);
                presentation.last_error = None;
            }
            Err(error) => presentation.record_error(error),
        }
    }
}

/// Updates opted-in root translations from their authoritative logical positions.
///
/// Schedule every render frame in `PostUpdate` before `TransformSystems::Propagate`.
/// Frames without a fixed tick retain their history and use the new overstep fraction.
/// Missing history or a missing `Time<Fixed>` resource causes direct synchronization.
/// Invalid presentation configurations record an error and leave the transform unchanged.
/// This system never changes logical positions, transform rotations, or transform scales.
pub fn sync_position_presentation_system(
    fixed_time: Option<Res<Time<Fixed>>>,
    mut query: PositionPresentationQuery,
) {
    let alpha = fixed_time.map(|time| f64::from(time.overstep_fraction().clamp(0.0, 1.0)));
    for (logical, transform, has_parent, mut presentation) in &mut query {
        let position = match validate_position(logical, transform.is_some(), has_parent) {
            Ok(position) => position,
            Err(error) => {
                presentation.record_error(error);
                continue;
            }
        };
        let Some(mut transform) = transform else {
            presentation.record_error(PositionPresentationError::MissingTransform);
            continue;
        };
        let translation = if presentation.interpolate
            && let (Some(previous), Some(alpha)) = (presentation.previous, alpha)
        {
            // Double precision avoids overflow between opposite finite f32 extremes.
            previous
                .as_dvec3()
                .lerp(position.as_dvec3(), alpha)
                .as_vec3()
        } else {
            position
        };
        if transform.translation != translation {
            transform.translation = translation;
        }
        if presentation.previous.is_none() {
            presentation.previous = Some(position);
        }
        presentation.last_error = None;
    }
}

fn validate_position(
    logical: Option<&LogicalPosition>,
    has_transform: bool,
    has_parent: bool,
) -> Result<Vec3, PositionPresentationError> {
    let position = logical
        .ok_or(PositionPresentationError::MissingLogicalPosition)?
        .0;
    if !has_transform {
        return Err(PositionPresentationError::MissingTransform);
    }
    if !position.is_finite() {
        return Err(PositionPresentationError::NonFiniteLogicalPosition);
    }
    if has_parent {
        return Err(PositionPresentationError::ParentedEntity);
    }
    Ok(position)
}
