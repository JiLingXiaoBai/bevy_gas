use super::super::{AbilitySpecHandle, ActiveAbilityHandle};
use super::AbilityTaskExecutionContext;
use crate::gameplay_targeting::AbilityActivationTargets;
use crate::unique_names::UniqueName;
use bevy::prelude::{Entity, Event};

/// Event emitted by an ability task completion action.
#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    context: AbilityTaskExecutionContext,
    targets: AbilityActivationTargets,
    active_ability: ActiveAbilityHandle,
    event_id: UniqueName,
}

impl AbilityTaskEvent {
    /// Creates an ability task event from the task's shared execution context.
    pub fn new(
        context: AbilityTaskExecutionContext,
        targets: AbilityActivationTargets,
        active_ability: ActiveAbilityHandle,
        event_id: UniqueName,
    ) -> Self {
        Self {
            context,
            targets,
            active_ability,
            event_id,
        }
    }

    /// Returns the task's shared execution context.
    pub fn get_context(&self) -> &AbilityTaskExecutionContext {
        &self.context
    }

    /// Returns the ability owner that started the task.
    pub fn get_source(&self) -> Entity {
        self.context.get_source()
    }

    /// Returns the complete target selection inherited from the ability activation.
    pub fn get_targets(&self) -> &AbilityActivationTargets {
        &self.targets
    }

    /// Returns the primary target inherited from the ability activation.
    pub fn get_target(&self) -> Entity {
        self.targets.get_primary_target()
    }

    /// Returns the active ability instance that owned the task.
    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    /// Returns the granted ability handle that owns the task.
    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.context.get_spec_handle()
    }

    /// Returns the event identifier supplied by the completion action.
    pub fn get_event_id(&self) -> UniqueName {
        self.event_id
    }

    /// Returns the captured ability level.
    pub fn get_level(&self) -> u32 {
        self.context.get_level()
    }
}
