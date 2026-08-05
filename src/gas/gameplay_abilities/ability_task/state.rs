use super::{AbilitySpecHandle, ActiveAbilityHandle};
use crate::gameplay_effects::GameplayEffect;
use crate::unique_names::UniqueName;
use bevy::prelude::{Component, Entity};
use std::sync::Arc;

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
