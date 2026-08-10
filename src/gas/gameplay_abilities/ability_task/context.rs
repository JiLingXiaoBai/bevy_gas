use super::AbilitySpecHandle;
use bevy::prelude::Entity;

/// Captures the shared gameplay values used while executing an ability task.
///
/// The context belongs to the task instance, while the task's completion action only describes
/// what should happen after the task finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityTaskExecutionContext {
    source: Entity,
    target: Entity,
    spec_handle: AbilitySpecHandle,
    level: u32,
}

impl AbilityTaskExecutionContext {
    /// Creates a task execution context.
    pub fn new(source: Entity, target: Entity, spec_handle: AbilitySpecHandle, level: u32) -> Self {
        Self {
            source,
            target,
            spec_handle,
            level,
        }
    }

    /// Returns the ability owner that started the task.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the task's captured fallback target.
    pub fn get_target(&self) -> Entity {
        self.target
    }

    /// Returns the granted ability handle that owns the task.
    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.spec_handle
    }

    /// Returns the captured ability level.
    pub fn get_level(&self) -> u32 {
        self.level
    }
}
