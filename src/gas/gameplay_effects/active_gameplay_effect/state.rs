use super::super::gameplay_effect_spec::{EffectDurationTicksSpec, GameplayEffectSpec};
use crate::modifiers::ModifierSourceId;
use bevy::prelude::*;

/// Stable generational handle for an active effect stored on its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActiveEffectHandle {
    target: Entity,
    slot: u32,
    generation: u32,
}

impl ActiveEffectHandle {
    /// Creates a handle from its target, slot, and generation.
    pub const fn new(target: Entity, slot: u32, generation: u32) -> Self {
        Self {
            target,
            slot,
            generation,
        }
    }

    /// Returns the entity that owns the active-effect container.
    pub const fn get_target(self) -> Entity {
        self.target
    }

    /// Returns the stable slot index within the target container.
    pub const fn get_slot(self) -> u32 {
        self.slot
    }

    /// Returns the slot generation used to reject stale handles.
    pub const fn get_generation(self) -> u32 {
        self.generation
    }
}

impl From<ActiveEffectHandle> for ModifierSourceId {
    fn from(handle: ActiveEffectHandle) -> Self {
        Self::new(handle.target.to_bits(), handle.slot, handle.generation)
    }
}

#[derive(Clone)]
struct ActiveEffectSlot {
    generation: u32,
    effect: Option<ActiveGameplayEffect>,
}

pub(super) enum ActiveEffectStorageError {
    CapacityExceeded { target: Entity },
}

/// Target-owned, stable-order storage for active gameplay effects.
#[derive(Component, Default)]
pub struct ActiveGameplayEffects {
    slots: Vec<ActiveEffectSlot>,
    free_slots: Vec<u32>,
}

impl ActiveGameplayEffects {
    /// Returns the number of active effects.
    pub fn len(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.effect.is_some())
            .count()
    }

    /// Returns whether the container has no active effects.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.effect.is_none())
    }

    /// Returns the active effect identified by `handle`.
    pub fn get(&self, handle: ActiveEffectHandle) -> Option<&ActiveGameplayEffect> {
        let slot = self.slots.get(handle.slot as usize)?;
        let effect = slot.effect.as_ref()?;
        (slot.generation == handle.generation && effect.get_target() == handle.target)
            .then_some(effect)
    }

    /// Returns the mutable active effect identified by `handle`.
    pub(crate) fn get_mut(
        &mut self,
        handle: ActiveEffectHandle,
    ) -> Option<&mut ActiveGameplayEffect> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        let effect = slot.effect.as_mut()?;
        (slot.generation == handle.generation && effect.get_target() == handle.target)
            .then_some(effect)
    }

    /// Iterates active handles in stable slot order for `target`.
    pub fn handles(&self, target: Entity) -> impl Iterator<Item = ActiveEffectHandle> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(move |(slot_index, slot)| {
                slot.effect
                    .as_ref()
                    .filter(|effect| effect.get_target() == target)
                    .map(|_| ActiveEffectHandle::new(target, slot_index as u32, slot.generation))
            })
    }

    pub(super) fn stored_handles(&self) -> impl Iterator<Item = ActiveEffectHandle> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot_index, slot)| {
                slot.effect.as_ref().map(|effect| {
                    ActiveEffectHandle::new(effect.get_target(), slot_index as u32, slot.generation)
                })
            })
    }

    pub(super) fn insert(
        &mut self,
        target: Entity,
        effect: ActiveGameplayEffect,
    ) -> Result<ActiveEffectHandle, ActiveEffectStorageError> {
        if let Some(slot_index) = self.free_slots.pop() {
            let slot = &mut self.slots[slot_index as usize];
            debug_assert!(slot.effect.is_none());
            slot.effect = Some(effect);
            return Ok(ActiveEffectHandle::new(target, slot_index, slot.generation));
        }

        let slot_index = u32::try_from(self.slots.len())
            .map_err(|_| ActiveEffectStorageError::CapacityExceeded { target })?;
        let generation = 1;
        self.slots.push(ActiveEffectSlot {
            generation,
            effect: Some(effect),
        });
        Ok(ActiveEffectHandle::new(target, slot_index, generation))
    }

    pub(super) fn remove(&mut self, handle: ActiveEffectHandle) -> Option<ActiveGameplayEffect> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        if slot.effect.as_ref()?.get_target() != handle.target {
            return None;
        }
        let effect = slot.effect.take()?;
        if slot.generation < u32::MAX {
            slot.generation += 1;
            self.free_slots.push(handle.slot);
        }
        Some(effect)
    }
}

/// Runtime state for a gameplay effect stored in [`ActiveGameplayEffects`].
#[derive(Clone)]
pub struct ActiveGameplayEffect {
    spec: GameplayEffectSpec,
    source: Entity,
    target: Entity,
    stack_count: u32,
    inhibited: bool,
    pub(super) duration: Option<ActiveEffectDurationTicks>,
    pub(super) period: Option<ActiveEffectPeriodTicks>,
}

impl ActiveGameplayEffect {
    pub(super) fn new(spec: GameplayEffectSpec, source: Entity, target: Entity) -> Self {
        let duration = match spec.get_duration_spec() {
            EffectDurationTicksSpec::DurationTicks(remain_ticks) => {
                Some(ActiveEffectDurationTicks {
                    remain_ticks: *remain_ticks,
                })
            }
            EffectDurationTicksSpec::Instant | EffectDurationTicksSpec::Infinite => None,
        };
        let period = spec
            .get_period_spec()
            .filter(|period| period.get_period_ticks() > 0)
            .map(|period| ActiveEffectPeriodTicks {
                period_ticks: period.get_period_ticks(),
                current_tick: 0,
            });
        Self {
            spec,
            source,
            target,
            stack_count: 1,
            inhibited: false,
            duration,
            period,
        }
    }

    /// Returns the captured effect specification.
    pub fn get_spec(&self) -> &GameplayEffectSpec {
        &self.spec
    }

    /// Returns the entity that applied the effect.
    pub fn get_source(&self) -> Entity {
        self.source
    }

    /// Returns the entity that owns the effect.
    pub fn get_target(&self) -> Entity {
        self.target
    }

    /// Returns the current stack count.
    pub fn get_stack_count(&self) -> u32 {
        self.stack_count
    }

    /// Sets a positive stack count.
    pub(crate) fn set_stack_count(&mut self, stack_count: u32) {
        self.stack_count = stack_count.max(1);
    }

    /// Returns whether ongoing tag requirements currently inhibit the effect.
    pub fn is_inhibited(&self) -> bool {
        self.inhibited
    }

    pub(super) fn set_inhibited(&mut self, inhibited: bool) {
        self.inhibited = inhibited;
    }

    /// Returns the optional remaining-duration state.
    pub fn get_duration(&self) -> Option<&ActiveEffectDurationTicks> {
        self.duration.as_ref()
    }

    /// Returns the optional period state.
    pub fn get_period(&self) -> Option<&ActiveEffectPeriodTicks> {
        self.period.as_ref()
    }
}

/// Remaining fixed ticks for a finite active effect.
#[derive(Debug, Clone, Copy)]
pub struct ActiveEffectDurationTicks {
    pub(super) remain_ticks: u32,
}

impl ActiveEffectDurationTicks {
    /// Returns the remaining fixed ticks.
    pub const fn get_remaining_ticks(&self) -> u32 {
        self.remain_ticks
    }
}

/// Fixed-tick state for a periodic active effect.
#[derive(Debug, Clone, Copy)]
pub struct ActiveEffectPeriodTicks {
    pub(super) period_ticks: u32,
    pub(super) current_tick: u32,
}

impl ActiveEffectPeriodTicks {
    /// Returns the configured period in fixed ticks.
    pub const fn get_period_ticks(&self) -> u32 {
        self.period_ticks
    }

    /// Returns the elapsed ticks in the current period.
    pub const fn get_current_tick(&self) -> u32 {
        self.current_tick
    }
}
