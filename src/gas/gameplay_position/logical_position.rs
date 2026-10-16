use bevy::prelude::{Component, Vec3};

/// The authoritative world-space position used by gameplay simulation and targeting.
///
/// The wrapped vector is independent of rendering transforms and parent-child transform
/// hierarchies. Update it during fixed-tick gameplay before spatial index synchronization.
/// Adding this component does not add a `Transform` or any other presentation component.
/// Non-finite positions are excluded from targeting and rejected by position presentation.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct LogicalPosition(pub Vec3);
