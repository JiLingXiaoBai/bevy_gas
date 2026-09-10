use super::{
    AbilitySpecHandle, AbilityTask, AbilityTaskExecutionContext, AbilityTaskOnFinished,
    ActiveAbilityHandle,
};
use crate::gameplay_effects::GameplayEffect;
use crate::unique_names::UniqueName;
use std::sync::Arc;

/// Describes an action to instantiate when an ability task finishes.
#[derive(Clone)]
pub enum AbilityTaskOnFinishedDef {
    /// Performs no follow-up action.
    None,
    /// Marks the active ability as ending.
    EndAbility,
    /// Dispatches actions in order, stopping at the first `EndAbility`.
    ///
    /// Previously queued gameplay requests remain queued. A batch is not a transaction.
    Batch {
        /// Ordered completion actions; nested batches preserve depth-first order.
        actions: Vec<AbilityTaskOnFinishedDef>,
    },
    /// Emits an [`AbilityTaskEvent`](super::AbilityTaskEvent).
    EmitEvent {
        /// Identifies the emitted event.
        event_id: UniqueName,
    },
    /// Applies an effect to the activation's primary target.
    ApplyGameplayEffectToTarget {
        /// Defines the effect to apply.
        effect: Arc<GameplayEffect>,
    },
    /// Applies an effect to every captured activation target.
    ApplyGameplayEffectToTargets {
        /// Defines the effect to apply.
        effect: Arc<GameplayEffect>,
    },
    /// Queues another ability activation using the task context.
    ActivateAbility {
        /// Identifies the ability to activate.
        handle: AbilitySpecHandle,
    },
}

/// Defines an ability task created when its owning ability starts.
#[derive(Clone)]
pub enum AbilityTaskDef {
    /// Dispatches the completion action during ability startup.
    Instant {
        /// Describes the action dispatched on completion.
        on_finished: AbilityTaskOnFinishedDef,
    },
    /// Creates a runtime task that completes after fixed task-processing ticks.
    WaitTicks {
        /// Number of task-processing ticks to wait.
        ticks: u32,
        /// Describes the action dispatched on completion.
        on_finished: AbilityTaskOnFinishedDef,
    },
}

impl AbilityTaskDef {
    /// Creates a task definition that completes during ability startup.
    pub fn instant(on_finished: AbilityTaskOnFinishedDef) -> Self {
        Self::Instant { on_finished }
    }

    /// Creates a task definition that waits for `ticks` task-processing ticks.
    pub fn wait_ticks(ticks: u32, on_finished: AbilityTaskOnFinishedDef) -> Self {
        Self::WaitTicks { ticks, on_finished }
    }

    /// Instantiates this definition for an active ability and shared execution context.
    pub fn instantiate(
        &self,
        active_ability: ActiveAbilityHandle,
        context: AbilityTaskExecutionContext,
    ) -> AbilityTask {
        match self {
            AbilityTaskDef::Instant { on_finished } => {
                AbilityTask::instant(active_ability, context, on_finished.instantiate())
            }
            AbilityTaskDef::WaitTicks { ticks, on_finished } => {
                AbilityTask::wait_ticks(active_ability, context, *ticks, on_finished.instantiate())
            }
        }
    }
}

impl AbilityTaskOnFinishedDef {
    pub(crate) fn instantiate(&self) -> AbilityTaskOnFinished {
        match self {
            AbilityTaskOnFinishedDef::None => AbilityTaskOnFinished::None,
            AbilityTaskOnFinishedDef::EndAbility => AbilityTaskOnFinished::EndAbility,
            AbilityTaskOnFinishedDef::Batch { actions } => AbilityTaskOnFinished::Batch {
                actions: actions.iter().map(Self::instantiate).collect(),
            },
            AbilityTaskOnFinishedDef::EmitEvent { event_id } => AbilityTaskOnFinished::EmitEvent {
                event_id: *event_id,
            },
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect } => {
                AbilityTaskOnFinished::ApplyGameplayEffect {
                    effect: effect.clone(),
                }
            }
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect } => {
                AbilityTaskOnFinished::ApplyGameplayEffectToTargets {
                    effect: effect.clone(),
                }
            }
            AbilityTaskOnFinishedDef::ActivateAbility { handle } => {
                AbilityTaskOnFinished::ActivateAbility { handle: *handle }
            }
        }
    }
}
