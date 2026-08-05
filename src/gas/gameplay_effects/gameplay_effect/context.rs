use crate::attributes::{AttributeIdManager, AttributeSet, AttributeSetSnapshot};
use crate::gameplay_tags::GameplayTagContainer;
use crate::modifiers::ModifierEvaluationContext;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Query;

pub struct EffectContext<'w, 's> {
    pub target: Option<Entity>,
    pub payload: &'w EffectPayload,
    pub attribute_id_manager: &'w AttributeIdManager,
    pub attr_set_query: &'w Query<'w, 's, &'static AttributeSet>,
    pub tag_container_query: &'w Query<'w, 's, &'static GameplayTagContainer>,
}

impl<'w, 's> EffectContext<'w, 's> {
    /// Returns the entity whose attributes and tags provide this effect's gameplay source.
    pub fn source(&self) -> Entity {
        self.payload.get_source()
    }

    /// Returns the entity that initiated the action producing this effect.
    pub fn instigator(&self) -> Entity {
        self.payload.get_instigator()
    }

    /// Returns the optional physical entity that directly caused this effect.
    pub fn causer(&self) -> Option<Entity> {
        self.payload.get_causer()
    }

    pub fn level(&self) -> u32 {
        self.payload.get_level()
    }

    pub fn source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.payload.get_source_snapshot()
    }

    /// Returns the global mapping used to locate hot and cold attributes.
    pub fn attribute_id_manager(&self) -> &AttributeIdManager {
        self.attribute_id_manager
    }
}

impl ModifierEvaluationContext for EffectContext<'_, '_> {
    fn target(&self) -> Option<Entity> {
        self.target
    }

    fn source(&self) -> Entity {
        self.payload.get_source()
    }

    fn instigator(&self) -> Entity {
        self.payload.get_instigator()
    }

    fn causer(&self) -> Option<Entity> {
        self.payload.get_causer()
    }

    fn level(&self) -> u32 {
        self.payload.get_level()
    }

    fn source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.payload.get_source_snapshot()
    }

    fn attribute_id_manager(&self) -> &AttributeIdManager {
        self.attribute_id_manager
    }

    fn source_tags(&self) -> Option<&GameplayTagContainer> {
        self.tag_container_query.get(self.payload.get_source()).ok()
    }

    fn target_tags(&self) -> Option<&GameplayTagContainer> {
        self.target
            .and_then(|target| self.tag_container_query.get(target).ok())
    }
}

#[derive(Clone)]
pub struct EffectPayload {
    source: Entity,
    instigator: Entity,
    causer: Option<Entity>,
    level: u32,
    source_snapshot: Option<AttributeSetSnapshot>,
}

impl EffectPayload {
    /// Creates effect metadata using `source` as both the gameplay source and default instigator.
    ///
    /// `causer` identifies the optional physical entity that directly caused the effect.
    pub fn new(source: Entity, causer: Option<Entity>, level: u32) -> Self {
        Self {
            source,
            instigator: source,
            causer,
            level,
            source_snapshot: None,
        }
    }

    /// Sets the entity that initiated the action producing this effect.
    pub fn with_instigator(mut self, instigator: Entity) -> Self {
        self.instigator = instigator;
        self
    }

    pub fn with_source_snapshot(mut self, source_snapshot: AttributeSetSnapshot) -> Self {
        self.source_snapshot = Some(source_snapshot);
        self
    }

    /// Returns the entity whose attributes and tags provide this effect's gameplay source.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the entity that initiated the action producing this effect.
    pub fn get_instigator(&self) -> Entity {
        self.instigator
    }

    /// Returns the optional physical entity that directly caused this effect.
    pub fn get_causer(&self) -> Option<Entity> {
        self.causer
    }

    pub fn get_level(&self) -> u32 {
        self.level
    }

    pub fn get_source_snapshot(&self) -> Option<&AttributeSetSnapshot> {
        self.source_snapshot.as_ref()
    }
}
