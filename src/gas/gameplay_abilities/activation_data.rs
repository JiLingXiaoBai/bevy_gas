use super::AbilityActivationContext;
use crate::gameplay_targeting::AbilityActivationTargets;
use bevy::prelude::Entity;

/// Immutable input shared by every stage of one ability activation.
///
/// This value keeps the ability owner, captured target selection, and
/// propagation context together while the ability moves from a queued request
/// into active runtime state.
#[derive(Clone)]
pub struct AbilityActivationData {
    source: Entity,
    targets: AbilityActivationTargets,
    context: AbilityActivationContext,
}

impl AbilityActivationData {
    /// Creates the shared data for one ability activation.
    ///
    /// # Parameters
    ///
    /// - `source`: Entity that owns the activated ability.
    /// - `targets`: Target selection captured for the activation.
    /// - `context`: Metadata describing how the activation was initiated.
    ///
    /// # Returns
    ///
    /// A new immutable activation data value.
    pub fn new(
        source: Entity,
        targets: impl Into<AbilityActivationTargets>,
        context: AbilityActivationContext,
    ) -> Self {
        Self {
            source,
            targets: targets.into(),
            context,
        }
    }

    /// Returns the entity that owns the activated ability.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the complete target selection captured for the activation.
    pub fn get_targets(&self) -> &AbilityActivationTargets {
        &self.targets
    }

    /// Returns the primary target captured for the activation.
    pub fn get_target(&self) -> Entity {
        self.targets.get_primary_target()
    }

    /// Returns the metadata captured for the activation.
    pub fn get_context(&self) -> &AbilityActivationContext {
        &self.context
    }
}
