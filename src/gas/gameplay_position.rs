//! Authoritative gameplay positions and optional one-way presentation synchronization.
//!
//! Logical positions are world-space gameplay state and require no rendering components.
//! Position presentation is explicitly enabled on a root entity; visual descendants inherit
//! its rendered transform through Bevy's normal transform propagation.

mod error;
mod logical_position;
mod plugin;
mod presentation;

pub use error::PositionPresentationError;
pub use logical_position::LogicalPosition;
pub use plugin::PositionPresentationPlugin;
pub use presentation::{
    PositionPresentation, capture_logical_position_history_system,
    sync_position_presentation_system,
};
