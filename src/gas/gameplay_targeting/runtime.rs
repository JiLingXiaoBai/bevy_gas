use super::spatial_index::{remove_discarded_target_position, remove_discarded_targetable};
use super::{
    TargetingRequestQueue, TargetingSpatialIndex, process_targeting_request_queue_system,
    sync_targeting_spatial_index_system, targeting_request_queue_has_work,
};
use crate::GameplayAbilitySystemSet;
use bevy::prelude::*;

/// Installs targeting resources, lifecycle observers, and systems in the runtime's phases.
///
/// Cross-domain phase ordering remains owned by the GAS runtime.
pub(crate) fn install_targeting(app: &mut App) {
    app.init_resource::<TargetingRequestQueue>()
        .init_resource::<TargetingSpatialIndex>()
        .add_observer(remove_discarded_targetable)
        .add_observer(remove_discarded_target_position)
        .add_systems(
            FixedUpdate,
            sync_targeting_spatial_index_system
                .in_set(GameplayAbilitySystemSet::TargetingSpatialSync),
        )
        .add_systems(
            FixedUpdate,
            process_targeting_request_queue_system
                .run_if(targeting_request_queue_has_work)
                .in_set(GameplayAbilitySystemSet::Targeting),
        );
}
