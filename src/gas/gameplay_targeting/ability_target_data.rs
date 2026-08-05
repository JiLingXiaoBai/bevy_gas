use bevy::prelude::*;

/// One spatial hit produced by a targeting request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityTargetHit {
    entity: Entity,
    position: Vec3,
    normal: Option<Vec3>,
}

impl AbilityTargetHit {
    /// Creates an entity hit at a world-space position.
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

/// Deterministically ordered target data produced by a targeting request.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityTargetData {
    origin: Vec3,
    hits: Vec<AbilityTargetHit>,
}

impl AbilityTargetData {
    /// Creates target data from a request origin and ordered hits.
    pub fn new(origin: Vec3, hits: Vec<AbilityTargetHit>) -> Self {
        Self { origin, hits }
    }

    /// Returns the world-space origin used by the targeting request.
    pub fn get_origin(&self) -> Vec3 {
        self.origin
    }

    /// Returns all hits in deterministic selection order.
    pub fn get_hits(&self) -> &[AbilityTargetHit] {
        &self.hits
    }

    /// Returns the first targeted entity, if any.
    pub fn primary_entity(&self) -> Option<Entity> {
        self.hits.first().map(AbilityTargetHit::get_entity)
    }

    /// Iterates over targeted entities in deterministic selection order.
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
