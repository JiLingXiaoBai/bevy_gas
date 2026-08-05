use super::{
    AbilityActivationContext, AbilityActivationStatus, AbilitySpecHandle, ActiveAbilityHandle,
    ActiveGameplayAbility,
};
use crate::gameplay_effects::{EffectPayload, GameplayEffect};
use crate::gameplay_execution::GameplayExecutionQueue;
use crate::unique_names::UniqueName;
use bevy::prelude::*;
use std::sync::Arc;

#[derive(Event, Clone)]
pub struct AbilityTaskEvent {
    source: Entity,
    target: Entity,
    active_ability: ActiveAbilityHandle,
    spec_handle: AbilitySpecHandle,
    event_id: UniqueName,
    level: u32,
}

impl AbilityTaskEvent {
    pub fn new(
        source: Entity,
        target: Entity,
        active_ability: ActiveAbilityHandle,
        spec_handle: AbilitySpecHandle,
        event_id: UniqueName,
        level: u32,
    ) -> Self {
        Self {
            source,
            target,
            active_ability,
            spec_handle,
            event_id,
            level,
        }
    }

    pub fn get_source(&self) -> Entity {
        self.source
    }

    pub fn get_target(&self) -> Entity {
        self.target
    }

    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    pub fn get_spec_handle(&self) -> AbilitySpecHandle {
        self.spec_handle
    }

    pub fn get_event_id(&self) -> UniqueName {
        self.event_id
    }

    pub fn get_level(&self) -> u32 {
        self.level
    }
}

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

#[derive(Clone)]
pub enum AbilityTaskOnFinished {
    None,
    EndAbility,
    EmitEvent {
        source: Entity,
        target: Entity,
        spec_handle: AbilitySpecHandle,
        event_id: UniqueName,
        level: u32,
    },
    ActivateAbility {
        source: Entity,
        target: Entity,
        handle: AbilitySpecHandle,
    },
    ApplyGameplayEffect {
        source: Entity,
        target: Entity,
        effect: Arc<GameplayEffect>,
        level: u32,
    },
    ApplyGameplayEffectToTargets {
        source: Entity,
        fallback_target: Entity,
        effect: Arc<GameplayEffect>,
        level: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum AbilityTaskKind {
    Instant,
    WaitTicks { remaining_ticks: u32 },
}

#[derive(Component, Clone)]
pub struct AbilityTask {
    active_ability: ActiveAbilityHandle,
    kind: AbilityTaskKind,
    on_finished: AbilityTaskOnFinished,
}

impl AbilityTask {
    pub fn instant(
        active_ability: ActiveAbilityHandle,
        on_finished: AbilityTaskOnFinished,
    ) -> Self {
        Self {
            active_ability,
            kind: AbilityTaskKind::Instant,
            on_finished,
        }
    }

    pub fn wait_ticks(
        active_ability: ActiveAbilityHandle,
        ticks: u32,
        on_finished: AbilityTaskOnFinished,
    ) -> Self {
        Self {
            active_ability,
            kind: AbilityTaskKind::WaitTicks {
                remaining_ticks: ticks,
            },
            on_finished,
        }
    }

    pub fn get_active_ability(&self) -> ActiveAbilityHandle {
        self.active_ability
    }

    pub fn get_kind(&self) -> AbilityTaskKind {
        self.kind
    }

    pub fn get_on_finished(&self) -> &AbilityTaskOnFinished {
        &self.on_finished
    }

    fn tick(&mut self) -> bool {
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

pub fn tick_ability_tasks_system(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut AbilityTask)>,
    mut active_ability_query: Query<&mut ActiveGameplayAbility>,
    mut execution_queue: ResMut<GameplayExecutionQueue>,
) {
    let mut task_entities: Vec<_> = task_query
        .iter_mut()
        .map(|(task_entity, _)| task_entity)
        .collect();
    task_entities.sort_by_key(|entity| entity.to_bits());

    for task_entity in task_entities {
        let (active_handle, active_context, on_finished) = {
            let Ok((_, mut task)) = task_query.get_mut(task_entity) else {
                continue;
            };
            let Ok(active_ability) = active_ability_query.get(task.get_active_ability()) else {
                commands.entity(task_entity).despawn();
                continue;
            };

            let active_status = active_ability.get_status();
            let active_context = active_ability.get_activation_context().clone();

            if !matches!(active_status, AbilityActivationStatus::Active) {
                commands.entity(task_entity).despawn();
                continue;
            }

            if !task.tick() {
                continue;
            }

            (
                task.get_active_ability(),
                active_context,
                task.get_on_finished().clone(),
            )
        };

        if matches!(
            dispatch_ability_task_completion(
                active_handle,
                on_finished,
                &active_context,
                &mut commands,
                &mut execution_queue,
            ),
            AbilityTaskCompletion::EndAbility
        ) && let Ok(mut active_ability) = active_ability_query.get_mut(active_handle)
        {
            active_ability.set_status(AbilityActivationStatus::Ending);
        }

        commands.entity(task_entity).despawn();
    }
}

pub(crate) enum AbilityTaskCompletion {
    Continue,
    EndAbility,
}

pub(crate) fn dispatch_ability_task_completion(
    active_ability: ActiveAbilityHandle,
    on_finished: AbilityTaskOnFinished,
    active_context: &AbilityActivationContext,
    commands: &mut Commands,
    execution_queue: &mut GameplayExecutionQueue,
) -> AbilityTaskCompletion {
    match on_finished {
        AbilityTaskOnFinished::None => {}
        AbilityTaskOnFinished::EndAbility => return AbilityTaskCompletion::EndAbility,
        AbilityTaskOnFinished::EmitEvent {
            source,
            target,
            spec_handle,
            event_id,
            level,
        } => {
            commands.trigger(AbilityTaskEvent::new(
                source,
                target,
                active_ability,
                spec_handle,
                event_id,
                level,
            ));
        }
        AbilityTaskOnFinished::ActivateAbility {
            source,
            target,
            handle,
        } => {
            if let Err(error) = execution_queue.push_chained_activation(
                source,
                target,
                handle,
                active_ability,
                active_context,
            ) {
                error!("failed to queue chained ability activation: {error}");
            }
        }
        AbilityTaskOnFinished::ApplyGameplayEffect {
            source,
            target,
            effect,
            level,
        } => {
            let payload = effect_payload_from_activation_context(source, level, active_context);
            execution_queue.push_application(target, effect, payload);
        }
        AbilityTaskOnFinished::ApplyGameplayEffectToTargets {
            source,
            fallback_target,
            effect,
            level,
        } => {
            let payload = effect_payload_from_activation_context(source, level, active_context);
            if let Some(target_data) = active_context.get_target_data() {
                for target in target_data.entities() {
                    execution_queue.push_application(target, effect.clone(), payload.clone());
                }
            } else {
                execution_queue.push_application(fallback_target, effect, payload);
            }
        }
    }
    AbilityTaskCompletion::Continue
}

fn effect_payload_from_activation_context(
    source: Entity,
    level: u32,
    activation_context: &AbilityActivationContext,
) -> EffectPayload {
    let payload = EffectPayload::new(source, activation_context.get_causer(), level)
        .with_instigator(activation_context.get_instigator());
    if let Some(source_snapshot) = activation_context.get_source_snapshot() {
        payload.with_source_snapshot(source_snapshot.clone())
    } else {
        payload
    }
}
