use super::{AbilitySpecHandle, AbilityTaskExecutionContext, ActiveAbilityHandle};
use crate::gameplay_effects::GameplayEffect;
use crate::unique_names::UniqueName;
use bevy::prelude::Component;
use std::sync::Arc;

/// Describes the action performed when an ability task finishes.
#[derive(Clone)]
pub enum AbilityTaskOnFinished {
    /// Performs no follow-up action.
    None,
    /// Marks the active ability as ending.
    EndAbility,
    /// Emits an [`AbilityTaskEvent`](super::AbilityTaskEvent).
    EmitEvent {
        /// Identifies the emitted event.
        event_id: UniqueName,
    },
    /// Queues another ability activation using the current task context.
    ActivateAbility {
        /// Identifies the ability to activate.
        handle: AbilitySpecHandle,
    },
    /// Applies an effect to the task context's fallback target.
    ApplyGameplayEffect {
        /// Defines the effect to apply.
        effect: Arc<GameplayEffect>,
    },
    /// Applies an effect to every captured activation target.
    ///
    /// The task context's target is used when the activation has no target data.
    ApplyGameplayEffectToTargets {
        /// Defines the effect to apply.
        effect: Arc<GameplayEffect>,
    },
}

/// Describes how an ability task advances over fixed ticks.
#[derive(Debug, Clone, Copy)]
pub enum AbilityTaskKind {
    /// Completes the next time task processing runs.
    Instant,
    /// Completes after the remaining number of task ticks reaches zero.
    WaitTicks { remaining_ticks: u32 },
}

/// Runtime state for a tick-driven ability task.
#[derive(Component, Clone)]
pub struct AbilityTask {
    active_ability: ActiveAbilityHandle,
    context: AbilityTaskExecutionContext,
    kind: AbilityTaskKind,
    on_finished: AbilityTaskOnFinished,
}

impl AbilityTask {
    /// Creates a task that completes the next time task processing runs.
    pub fn instant(
        active_ability: ActiveAbilityHandle,
        context: AbilityTaskExecutionContext,
        on_finished: AbilityTaskOnFinished,
    ) -> Self {
        Self {
            active_ability,
            context,
            kind: AbilityTaskKind::Instant,
            on_finished,
        }
    }

    /// Creates a task that completes after `ticks` task-processing ticks.
    pub fn wait_ticks(
        active_ability: ActiveAbilityHandle,
        context: AbilityTaskExecutionContext,
        ticks: u32,
        on_finished: AbilityTaskOnFinished,
    ) -> Self {
        Self {
            active_ability,
            context,
            kind: AbilityTaskKind::WaitTicks {
                remaining_ticks: ticks,
            },
            on_finished,
        }
    }

    /// Returns the active ability instance that owns this task.
    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    /// Returns the captured execution context shared by completion actions.
    pub fn get_context(&self) -> &AbilityTaskExecutionContext {
        &self.context
    }

    /// Returns the task's tick behavior.
    pub fn get_kind(&self) -> AbilityTaskKind {
        self.kind
    }

    /// Returns the action performed when the task finishes.
    pub fn get_on_finished(&self) -> &AbilityTaskOnFinished {
        &self.on_finished
    }

    pub(super) fn tick(&mut self) -> bool {
        match &mut self.kind {
            AbilityTaskKind::Instant => true,
            AbilityTaskKind::WaitTicks { remaining_ticks } => {
                if *remaining_ticks > 0 {
                    *remaining_ticks -= 1;
                }
                *remaining_ticks == 0
            }
        }
    }
}
