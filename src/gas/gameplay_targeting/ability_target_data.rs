use bevy::prelude::*;

/// One entity target with spatial metadata supplied by a game adapter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityTargetHit {
    entity: Entity,
    position: Vec3,
    normal: Option<Vec3>,
}

impl AbilityTargetHit {
    /// Creates a target for `entity` with a game-supplied world-space `position` and optional normal.
    ///
    /// The position may be an aim point or a contact point, as defined by the producer. This
    /// constructor performs no entity, geometry, or numeric validation and requires no components.
    pub fn new(entity: Entity, position: Vec3, normal: Option<Vec3>) -> Self {
        Self {
            entity,
            position,
            normal,
        }
    }

    /// Returns the targeted entity.
    pub fn get_entity(&self) -> Entity {
        self.entity
    }

    /// Returns the world-space hit position.
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    /// Returns the optional world-space surface normal.
    pub fn get_normal(&self) -> Option<Vec3> {
        self.normal
    }
}

/// Captured target data in the order supplied by the game.
///
/// Games own selection, stable ordering, duplicate handling, and geometry validation. GAS does
/// not run a spatial query or revalidate contact when consuming this data.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityTargetData {
    origin: Vec3,
    hits: Vec<AbilityTargetHit>,
}

impl AbilityTargetData {
    /// Creates target data from a world-space `origin` and ordered `hits`.
    ///
    /// Returns the supplied data unchanged; it does not sort, deduplicate, or validate hits.
    pub fn new(origin: Vec3, hits: Vec<AbilityTargetHit>) -> Self {
        Self { origin, hits }
    }

    /// Returns the world-space origin used by the targeting request.
    pub fn get_origin(&self) -> Vec3 {
        self.origin
    }

    /// Returns all hits in the order supplied by the producer.
    pub fn get_hits(&self) -> &[AbilityTargetHit] {
        &self.hits
    }

    /// Returns the first targeted entity, if any.
    pub fn primary_entity(&self) -> Option<Entity> {
        self.hits.first().map(AbilityTargetHit::get_entity)
    }

    /// Iterates over targeted entities in the order supplied by the producer.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.hits.iter().map(AbilityTargetHit::get_entity)
    }

    /// Returns whether no targets were acquired.
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Returns the number of acquired targets.
    pub fn len(&self) -> usize {
        self.hits.len()
    }
}
