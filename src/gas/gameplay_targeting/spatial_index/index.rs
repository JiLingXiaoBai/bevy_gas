use super::super::Targetable;
use super::tree::DynamicAabbTree;
use crate::gameplay_position::LogicalPosition;
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

/// Derived dynamic AABB index of finite logical world-space positions of targetable entities.
///
/// The runtime plugin synchronizes this resource before its targeting phase. The index
/// stores spatial proxies only; gameplay tags and attributes remain in ECS components.
/// Range acquisition requires at least one synchronization and must run before subsequent
/// spatial mutations. Install the runtime plugin to retain removal tracking across frames.
#[derive(Resource, Default)]
pub struct TargetingSpatialIndex {
    tree: DynamicAabbTree,
    proxies: EntityHashMap<usize>,
    initialized: bool,
}

impl TargetingSpatialIndex {
    /// Returns the number of indexed entities with finite positions.
    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    /// Returns whether the index contains no targetable entities.
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }

    /// Returns whether the index has completed its initial synchronization.
    ///
    /// This does not certify that external movement systems have run in the correct order.
    pub fn is_ready(&self) -> bool {
        self.initialized
    }

    pub(crate) fn query(&self, origin: Vec3, radius: f32, output: &mut Vec<Entity>) {
        self.tree.query(origin, radius, output);
    }

    fn update(&mut self, entity: Entity, position: Vec3) {
        if !position.is_finite() {
            self.remove(entity);
            return;
        }
        if let Some(&leaf) = self.proxies.get(&entity) {
            self.tree.update(leaf, position);
        } else {
            let leaf = self.tree.insert(entity, position);
            self.proxies.insert(entity, leaf);
        }
    }

    fn remove(&mut self, entity: Entity) {
        if let Some(leaf) = self.proxies.remove(&entity) {
            self.tree.remove(leaf);
        }
    }
}

type ChangedTargets<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static LogicalPosition),
    (
        With<Targetable>,
        Or<(Changed<LogicalPosition>, Changed<Targetable>)>,
    ),
>;

/// Synchronizes target additions, spatial changes, and removals into the dynamic tree.
///
/// Run after logical gameplay movement and structural commands,
/// and before any range acquisition. The first run indexes all eligible entities; later
/// runs update changed components and remove discarded proxies. Non-finite positions are
/// excluded until changed to finite values. This system must run even when no requests exist.
/// The runtime plugin also installs lifecycle observers so removals cannot expire between
/// fixed ticks; manually scheduled users should retain that plugin's resource registration.
pub fn sync_targeting_spatial_index_system(
    mut index: ResMut<TargetingSpatialIndex>,
    all_targets: Query<(Entity, &LogicalPosition), With<Targetable>>,
    changed_targets: ChangedTargets,
    mut removed_targetable: RemovedComponents<Targetable>,
    mut removed_positions: RemovedComponents<LogicalPosition>,
) {
    // Process removals first: removing and re-adding a component must reinsert its proxy.
    for entity in removed_targetable.read().chain(removed_positions.read()) {
        index.remove(entity);
    }
    if !index.initialized {
        for (entity, position) in &all_targets {
            index.update(entity, position.0);
        }
        index.initialized = true;
    } else {
        for (entity, position) in &changed_targets {
            index.update(entity, position.0);
        }
    }
}

/// Keeps removals effective even when no fixed tick runs before removal messages expire.
pub(crate) fn remove_discarded_targetable(
    event: On<Discard<Targetable>>,
    mut index: ResMut<TargetingSpatialIndex>,
) {
    index.remove(event.entity);
}

/// Component replacement also discards the old proxy; change detection reinserts it.
pub(crate) fn remove_discarded_target_position(
    event: On<Discard<LogicalPosition>>,
    mut index: ResMut<TargetingSpatialIndex>,
) {
    index.remove(event.entity);
}
