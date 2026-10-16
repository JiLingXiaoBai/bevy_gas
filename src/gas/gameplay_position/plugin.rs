use super::presentation::reset_discarded_logical_position_presentation;
use super::{capture_logical_position_history_system, sync_position_presentation_system};
use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// Installs optional synchronization from logical positions to root rendering transforms.
///
/// This plugin can be installed without GAS. It captures interpolation history in
/// `FixedFirst`, presents it in `PostUpdate` before transform propagation, and clears history
/// when a logical-position component is discarded. Only entities with
/// [`super::PositionPresentation`] are affected.
///
/// Install Bevy's `TransformPlugin` (included in `DefaultPlugins`) to propagate the resulting
/// transforms. Without `Time<Fixed>`, presentation directly copies the logical position.
pub struct PositionPresentationPlugin;

impl Plugin for PositionPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedFirst, capture_logical_position_history_system)
            .add_systems(
                PostUpdate,
                sync_position_presentation_system.before(TransformSystems::Propagate),
            )
            .add_observer(reset_discarded_logical_position_presentation);
    }
}
