use super::{AbilitySpecHandle, AbilityTask, AbilityTaskOnFinished, ActiveAbilityHandle};
use crate::gameplay_effects::GameplayEffect;
use crate::unique_names::UniqueName;
use bevy::prelude::Entity;
use std::sync::Arc;

#[derive(Clone)]
pub enum AbilityTaskOnFinishedDef {
    None,
    EndAbility,
    EmitEvent {
        event_id: UniqueName,
    },
    ApplyGameplayEffectToTarget {
        effect: Arc<GameplayEffect>,
    },
    /// Applies an effect to every entity in the activation target data.
    ///
    /// Falls back to the ability's legacy single target when no target data is attached.
    ApplyGameplayEffectToTargets {
        effect: Arc<GameplayEffect>,
    },
    ActivateAbility {
        handle: AbilitySpecHandle,
    },
}

#[derive(Clone)]
pub enum AbilityTaskDef {
    Instant {
        on_finished: AbilityTaskOnFinishedDef,
    },
    WaitTicks {
        ticks: u32,
        on_finished: AbilityTaskOnFinishedDef,
    },
}

impl AbilityTaskDef {
    pub fn instant(on_finished: AbilityTaskOnFinishedDef) -> Self {
        Self::Instant { on_finished }
    }

    pub fn wait_ticks(ticks: u32, on_finished: AbilityTaskOnFinishedDef) -> Self {
        Self::WaitTicks { ticks, on_finished }
    }

    pub fn instantiate(
        &self,
        active_ability: ActiveAbilityHandle,
        source: Entity,
        target: Entity,
        spec_handle: AbilitySpecHandle,
        level: u32,
    ) -> AbilityTask {
        match self {
            AbilityTaskDef::Instant { on_finished } => AbilityTask::instant(
                active_ability,
                on_finished.instantiate(source, target, spec_handle, level),
            ),
            AbilityTaskDef::WaitTicks { ticks, on_finished } => AbilityTask::wait_ticks(
                active_ability,
                *ticks,
                on_finished.instantiate(source, target, spec_handle, level),
            ),
        }
    }
}

impl AbilityTaskOnFinishedDef {
    pub(crate) fn instantiate(
        &self,
        source: Entity,
        target: Entity,
        spec_handle: AbilitySpecHandle,
        level: u32,
    ) -> AbilityTaskOnFinished {
        match self {
            AbilityTaskOnFinishedDef::None => AbilityTaskOnFinished::None,
            AbilityTaskOnFinishedDef::EndAbility => AbilityTaskOnFinished::EndAbility,
            AbilityTaskOnFinishedDef::EmitEvent { event_id } => AbilityTaskOnFinished::EmitEvent {
                source,
                target,
                spec_handle,
                event_id: *event_id,
                level,
            },
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect } => {
                AbilityTaskOnFinished::ApplyGameplayEffect {
                    source,
                    target,
                    effect: effect.clone(),
                    level,
                }
            }
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTargets { effect } => {
                AbilityTaskOnFinished::ApplyGameplayEffectToTargets {
                    source,
                    fallback_target: target,
                    effect: effect.clone(),
                    level,
                }
            }
            AbilityTaskOnFinishedDef::ActivateAbility { handle } => {
                AbilityTaskOnFinished::ActivateAbility {
                    source,
                    target,
                    handle: *handle,
                }
            }
        }
    }
}
