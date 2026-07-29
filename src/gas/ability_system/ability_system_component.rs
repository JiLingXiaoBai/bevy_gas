use crate::attributes::{AttributeIdManager, AttributeSet, AttributeSetSnapshot};
use crate::gameplay_abilities::{
    AbilityActivationContext, AbilityActivationStatus, AbilityChainError, AbilitySpecHandle,
    AbilityTaskDef, ActiveAbilityHandle, ActiveGameplayAbility, GameplayAbility,
    GameplayAbilitySpec,
};
use crate::gameplay_effects::{
    ActiveEffectDurationTicks, ActiveEffectPeriodTicks, ActiveGameplayEffect,
    ActiveGameplayEffectTargetIndex, EffectContext, GameplayEffectApplicationPlan,
};
use crate::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagManager, tag_bits_from_tags_with_manager,
};
use crate::randoms::Random;
use crate::{
    EffectPayload, apply_gameplay_effect, execute_gameplay_effect_plan, prepare_gameplay_effect,
};
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

#[derive(SystemParam)]
pub struct AbilitySystemParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub tag_manager: Res<'w, GameplayTagManager>,
    pub random_gen: ResMut<'w, Random>,
    pub attribute_id_manager: Res<'w, AttributeIdManager>,
    pub attr_set_query: Query<'w, 's, &'static mut AttributeSet>,
    pub tag_container_query: Query<'w, 's, &'static mut GameplayTagContainer>,
    pub asc_query: Query<'w, 's, &'static mut AbilitySystemComponent>,
    pub attr_set_snapshot_query: Query<'w, 's, &'static AttributeSetSnapshot>,
    pub active_effect_target_index: ResMut<'w, ActiveGameplayEffectTargetIndex>,
    pub active_effect_query: Query<
        'w,
        's,
        (
            Entity,
            &'static mut ActiveGameplayEffect,
            Option<&'static mut ActiveEffectDurationTicks>,
            Option<&'static mut ActiveEffectPeriodTicks>,
        ),
    >,
    pub active_ability_query: Query<'w, 's, (Entity, &'static mut ActiveGameplayAbility)>,
    pub time: Res<'w, Time>,
}

#[derive(Component, Default)]
pub struct AbilitySystemComponent {
    next_ability_handle: u32,
    abilities: Vec<GameplayAbilitySpec>,
    ability_indices: HashMap<AbilitySpecHandle, usize>,
    blocked_ability_tags: GameplayTagContainer,
}

impl AbilitySystemComponent {
    pub fn give_ability(
        &mut self,
        ability: Arc<GameplayAbility>,
        level: u32,
        input_id: Option<u16>,
    ) -> AbilitySpecHandle {
        let handle = AbilitySpecHandle::new(self.next_ability_handle);
        self.next_ability_handle = self.next_ability_handle.wrapping_add(1);
        let index = self.abilities.len();
        self.abilities
            .push(GameplayAbilitySpec::new(handle, ability, level, input_id));
        self.ability_indices.insert(handle, index);
        handle
    }

    pub fn clear_ability(&mut self, handle: AbilitySpecHandle) -> bool {
        if self
            .find_ability_spec(handle)
            .is_some_and(|spec| spec.get_active_count() > 0)
        {
            return false;
        }

        let old_len = self.abilities.len();
        self.abilities.retain(|spec| spec.get_handle() != handle);
        let removed = old_len != self.abilities.len();
        if removed {
            self.rebuild_ability_indices();
        }
        removed
    }

    pub fn get_ability_specs(&self) -> &[GameplayAbilitySpec] {
        &self.abilities
    }

    pub fn get_blocked_ability_tags(&self) -> &GameplayTagContainer {
        &self.blocked_ability_tags
    }

    pub fn find_ability_spec(&self, handle: AbilitySpecHandle) -> Option<&GameplayAbilitySpec> {
        self.ability_indices
            .get(&handle)
            .and_then(|&index| self.abilities.get(index))
    }

    fn find_ability_spec_mut(
        &mut self,
        handle: AbilitySpecHandle,
    ) -> Option<&mut GameplayAbilitySpec> {
        self.ability_indices
            .get(&handle)
            .and_then(|&index| self.abilities.get_mut(index))
    }

    fn rebuild_ability_indices(&mut self) {
        self.ability_indices.clear();
        for (index, spec) in self.abilities.iter().enumerate() {
            self.ability_indices.insert(spec.get_handle(), index);
        }
    }

    fn start_ability(
        &mut self,
        source: Entity,
        target: Entity,
        spec_handle: AbilitySpecHandle,
        activation_context: AbilityActivationContext,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
    ) -> ActiveAbilityHandle {
        let blocked_tags = self
            .find_ability_spec(spec_handle)
            .map(|spec| {
                spec.get_ability()
                    .get_tags()
                    .get_block_abilities_with_tags()
                    .to_vec()
            })
            .unwrap_or_default();
        self.blocked_ability_tags
            .add_tags(&blocked_tags, tag_manager);

        if let Some(spec) = self.find_ability_spec_mut(spec_handle) {
            spec.increment_active_count();
        }

        let mut entity_cmds = commands.spawn(ActiveGameplayAbility::new(
            source,
            spec_handle,
            target,
            AbilityActivationStatus::Active,
            activation_context,
        ));
        let active_handle = entity_cmds.id();
        entity_cmds.set_parent_in_place(source);

        active_handle
    }

    fn finish_active_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        active_ability: &ActiveGameplayAbility,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
    ) -> bool {
        self.rollback_started_ability(
            active_handle,
            active_ability.get_spec_handle(),
            commands,
            tag_manager,
        )
    }

    fn rollback_started_ability(
        &mut self,
        active_handle: ActiveAbilityHandle,
        spec_handle: AbilitySpecHandle,
        commands: &mut Commands,
        tag_manager: &Res<GameplayTagManager>,
    ) -> bool {
        let blocked_tags = self
            .find_ability_spec(spec_handle)
            .map(|spec| {
                spec.get_ability()
                    .get_tags()
                    .get_block_abilities_with_tags()
                    .to_vec()
            })
            .unwrap_or_default();

        if let Some(spec) = self.find_ability_spec_mut(spec_handle) {
            spec.decrement_active_count();
        }

        self.blocked_ability_tags
            .remove_tags(&blocked_tags, tag_manager);
        commands.entity(active_handle).despawn_children().despawn();
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityActivationError {
    InvalidChain(AbilityChainError),
    MissingAbilitySystemComponent {
        source: Entity,
    },
    AbilityNotFound {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    MultipleInstancesNotAllowed {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    ActivationRequirementsNotMet {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    CommitPreparationFailed {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    StartFailed {
        source: Entity,
        handle: AbilitySpecHandle,
    },
    CommitExecutionFailed {
        source: Entity,
        handle: AbilitySpecHandle,
    },
}

impl fmt::Display for AbilityActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbilityActivationError::InvalidChain(err) => {
                write!(f, "ability activation failed: {err}")
            }
            AbilityActivationError::MissingAbilitySystemComponent { source } => write!(
                f,
                "ability activation failed: source entity {source:?} has no AbilitySystemComponent"
            ),
            AbilityActivationError::AbilityNotFound { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} has no ability handle {}",
                handle.get_value()
            ),
            AbilityActivationError::MultipleInstancesNotAllowed { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} is already active",
                handle.get_value()
            ),
            AbilityActivationError::ActivationRequirementsNotMet { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} does not meet activation requirements",
                handle.get_value()
            ),
            AbilityActivationError::CommitPreparationFailed { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not prepare cost or cooldown",
                handle.get_value()
            ),
            AbilityActivationError::StartFailed { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not start",
                handle.get_value()
            ),
            AbilityActivationError::CommitExecutionFailed { source, handle } => write!(
                f,
                "ability activation failed: source entity {source:?} ability handle {} could not execute cost or cooldown",
                handle.get_value()
            ),
        }
    }
}

impl Error for AbilityActivationError {}

fn ability_activation_failed(err: AbilityActivationError) -> Result<(), AbilityActivationError> {
    error!("{err}");
    Err(err)
}

pub fn try_activate_ability_by_handle(
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    activation_context: AbilityActivationContext,
    params: &mut AbilitySystemParams,
) -> Result<(), AbilityActivationError> {
    if let Some(chain) = activation_context.get_chain()
        && let Err(err) = chain.validate_for_handle(handle)
    {
        return ability_activation_failed(AbilityActivationError::InvalidChain(err));
    }

    let (ability, level) = {
        let Ok(asc) = params.asc_query.get(source) else {
            return ability_activation_failed(
                AbilityActivationError::MissingAbilitySystemComponent { source },
            );
        };
        let Some(spec) = asc.find_ability_spec(handle) else {
            return ability_activation_failed(AbilityActivationError::AbilityNotFound {
                source,
                handle,
            });
        };
        (spec.get_ability().clone(), spec.get_level())
    };

    cancel_active_abilities_with_tags(
        source,
        ability.get_tags().get_cancel_abilities_with_tags(),
        params,
    );

    let active_count = {
        let Ok(asc) = params.asc_query.get(source) else {
            return ability_activation_failed(
                AbilityActivationError::MissingAbilitySystemComponent { source },
            );
        };
        let Some(spec) = asc.find_ability_spec(handle) else {
            return ability_activation_failed(AbilityActivationError::AbilityNotFound {
                source,
                handle,
            });
        };
        spec.get_active_count()
    };

    if !ability.allow_multiple_instances() && active_count > 0 {
        return ability_activation_failed(AbilityActivationError::MultipleInstancesNotAllowed {
            source,
            handle,
        });
    }

    if !passes_ability_activation_requirements(source, &ability, params) {
        return ability_activation_failed(AbilityActivationError::ActivationRequirementsNotMet {
            source,
            handle,
        });
    }

    let Some(commit_plans) =
        prepare_ability_commit_plans(source, &ability, level, Some(&activation_context), params)
    else {
        return ability_activation_failed(AbilityActivationError::CommitPreparationFailed {
            source,
            handle,
        });
    };

    let active_handle = {
        let Ok(mut asc) = params.asc_query.get_mut(source) else {
            return ability_activation_failed(AbilityActivationError::StartFailed {
                source,
                handle,
            });
        };
        asc.start_ability(
            source,
            target,
            handle,
            activation_context.clone(),
            &mut params.commands,
            &params.tag_manager,
        )
    };

    if !execute_ability_commit_plans(commit_plans, params) {
        if let Ok(mut asc) = params.asc_query.get_mut(source) {
            asc.rollback_started_ability(
                active_handle,
                handle,
                &mut params.commands,
                &params.tag_manager,
            );
        }
        return ability_activation_failed(AbilityActivationError::CommitExecutionFailed {
            source,
            handle,
        });
    }

    for effect in ability.get_activation_effects() {
        // Activation effects are best-effort; cost/cooldown commit already decided activation success.
        let payload = effect_payload_from_activation_context(source, level, &activation_context);
        let _ = apply_gameplay_effect(target, effect, params, &payload);
    }

    spawn_startup_ability_tasks(
        active_handle,
        source,
        target,
        handle,
        level,
        ability.get_startup_tasks(),
        &mut params.commands,
    );

    if ability.should_end_on_activation() {
        params
            .commands
            .entity(active_handle)
            .insert(ActiveGameplayAbility::new(
                source,
                handle,
                target,
                AbilityActivationStatus::Ending,
                activation_context,
            ));
    }

    Ok(())
}

fn spawn_startup_ability_tasks(
    active_handle: ActiveAbilityHandle,
    source: Entity,
    target: Entity,
    spec_handle: AbilitySpecHandle,
    level: u32,
    startup_tasks: &[AbilityTaskDef],
    commands: &mut Commands,
) {
    for task_def in startup_tasks {
        let mut task_cmds =
            commands.spawn(task_def.instantiate(active_handle, source, target, spec_handle, level));
        task_cmds.set_parent_in_place(active_handle);
    }
}

pub fn end_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Ending,
        params,
    )
}

pub fn cancel_ability(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    params: &mut AbilitySystemParams,
) -> bool {
    finish_ability_with_status(
        source,
        active_handle,
        AbilityActivationStatus::Cancelled,
        params,
    )
}

pub fn can_activate_ability(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    if !passes_ability_activation_requirements(source, ability, params) {
        return false;
    }

    can_pay_ability_cost(source, target, ability, level, params)
}

fn passes_ability_activation_requirements(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    params: &mut AbilitySystemParams,
) -> bool {
    if let Ok(asc) = params.asc_query.get(source)
        && asc
            .blocked_ability_tags
            .has_any(ability.get_tags().get_ability_asset_tags())
    {
        return false;
    }

    let source_tags = params.tag_container_query.get(source).ok();
    if let Some(tags) = source_tags {
        let ability_tags = ability.get_tags();
        if tags.has_any(ability_tags.get_activation_blocked_tags()) {
            return false;
        }
        if !tags.has_all(ability_tags.get_activation_required_tags()) {
            return false;
        }

        if let Some(cooldown_def) = ability.get_cooldown()
            && tags.has_any(cooldown_def.get_tags().get_granted_tags())
        {
            return false;
        }
    }

    true
}

pub fn commit_ability(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    let Some(plans) = prepare_ability_commit_plans(source, ability, level, None, params) else {
        return false;
    };

    execute_ability_commit_plans(plans, params)
}

struct AbilityCommitPlans {
    cost_plan: Option<GameplayEffectApplicationPlan>,
    cooldown_plan: Option<GameplayEffectApplicationPlan>,
}

fn prepare_ability_commit_plans(
    source: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    activation_context: Option<&AbilityActivationContext>,
    params: &mut AbilitySystemParams,
) -> Option<AbilityCommitPlans> {
    let cost_plan = if let Some(cost_def) = ability.get_cost() {
        if !cost_def.has_only_add_modifiers() {
            return None;
        }
        let payload =
            effect_payload_from_optional_activation_context(source, level, activation_context);
        let plan = prepare_gameplay_effect(source, cost_def, params, &payload)?;
        if !plan.is_instant() {
            return None;
        }
        if !can_pay_prepared_cost(source, &plan, params) {
            return None;
        }
        Some(plan)
    } else {
        None
    };

    let cooldown_plan = if let Some(cooldown_def) = ability.get_cooldown() {
        let payload =
            effect_payload_from_optional_activation_context(source, level, activation_context);
        let plan = prepare_gameplay_effect(source, cooldown_def, params, &payload)?;
        Some(plan)
    } else {
        None
    };

    Some(AbilityCommitPlans {
        cost_plan,
        cooldown_plan,
    })
}

fn effect_payload_from_optional_activation_context(
    source: Entity,
    level: u32,
    activation_context: Option<&AbilityActivationContext>,
) -> EffectPayload {
    activation_context.map_or_else(
        || EffectPayload::new(source, None, level),
        |activation_context| {
            effect_payload_from_activation_context(source, level, activation_context)
        },
    )
}

fn effect_payload_from_activation_context(
    source: Entity,
    level: u32,
    activation_context: &AbilityActivationContext,
) -> EffectPayload {
    let payload = EffectPayload::new(source, activation_context.get_causer(), level);
    if let Some(source_snapshot) = activation_context.get_source_snapshot() {
        payload.with_source_snapshot(source_snapshot.clone())
    } else {
        payload
    }
}

fn execute_ability_commit_plans(
    plans: AbilityCommitPlans,
    params: &mut AbilitySystemParams,
) -> bool {
    if let Some(plan) = plans.cost_plan
        && !execute_gameplay_effect_plan(plan, params)
    {
        return false;
    }

    if let Some(plan) = plans.cooldown_plan
        && !execute_gameplay_effect_plan(plan, params)
    {
        return false;
    }

    true
}

fn can_pay_prepared_cost(
    source: Entity,
    cost_plan: &GameplayEffectApplicationPlan,
    params: &mut AbilitySystemParams,
) -> bool {
    let Ok(mut attr_set) = params.attr_set_query.get_mut(source) else {
        return false;
    };

    for cost in cost_plan.get_modifier_specs() {
        let Some(current_val) =
            attr_set.get_current_value(&params.attribute_id_manager, cost.get_id())
        else {
            return false;
        };
        if current_val + cost.get_value() < 0.0 {
            return false;
        }
    }

    true
}

fn finish_ability_with_status(
    source: Entity,
    active_handle: ActiveAbilityHandle,
    status: AbilityActivationStatus,
    params: &mut AbilitySystemParams,
) -> bool {
    let Ok((_, mut active_ability)) = params.active_ability_query.get_mut(active_handle) else {
        return false;
    };
    if active_ability.get_source() != source {
        return false;
    }
    active_ability.set_status(status);
    true
}

pub fn cleanup_finished_abilities_system(
    mut commands: Commands,
    active_ability_query: Query<(Entity, &ActiveGameplayAbility)>,
    mut asc_query: Query<&mut AbilitySystemComponent>,
    tag_manager: Res<GameplayTagManager>,
) {
    for (active_handle, active_ability) in active_ability_query.iter() {
        if !matches!(
            active_ability.get_status(),
            AbilityActivationStatus::Ending | AbilityActivationStatus::Cancelled
        ) {
            continue;
        }

        if let Ok(mut asc) = asc_query.get_mut(active_ability.get_source()) {
            asc.finish_active_ability(active_handle, active_ability, &mut commands, &tag_manager);
        } else {
            commands.entity(active_handle).despawn_children().despawn();
        }
    }
}

fn cancel_active_abilities_with_tags(
    source: Entity,
    tags: &[GameplayTag],
    params: &mut AbilitySystemParams,
) {
    if tags.is_empty() {
        return;
    }

    let active_handles: Vec<_> = {
        let Ok(asc) = params.asc_query.get(source) else {
            return;
        };
        params
            .active_ability_query
            .iter()
            .filter_map(|(active_handle, active)| {
                if active.get_source() != source {
                    return None;
                }
                let spec = asc.find_ability_spec(active.get_spec_handle())?;
                ability_has_any_tags(spec.get_ability(), tags, &params.tag_manager)
                    .then_some(active_handle)
            })
            .collect()
    };

    for active_handle in active_handles {
        let Ok((_, active_ability)) = params.active_ability_query.get_mut(active_handle) else {
            continue;
        };
        let active_ability = active_ability.clone();
        if let Ok(mut asc) = params.asc_query.get_mut(source) {
            asc.finish_active_ability(
                active_handle,
                &active_ability,
                &mut params.commands,
                &params.tag_manager,
            );
        }
    }
}

fn can_pay_ability_cost(
    source: Entity,
    target: Entity,
    ability: &Arc<GameplayAbility>,
    level: u32,
    params: &mut AbilitySystemParams,
) -> bool {
    let Some(cost_def) = ability.get_cost() else {
        return true;
    };
    if !cost_def.has_only_add_modifiers() {
        return false;
    }

    let payload = EffectPayload::new(source, None, level);
    let cost_spec = {
        let context = EffectContext {
            target: Some(target),
            payload: &payload,
            attribute_id_manager: &params.attribute_id_manager,
            attr_set_query: &params.attr_set_query.as_readonly(),
            tag_container_query: &params.tag_container_query.as_readonly(),
            asc_query: &params.asc_query.as_readonly(),
        };

        cost_def.make_spec(&context)
    };

    let Ok(mut attr_set) = params.attr_set_query.get_mut(source) else {
        return false;
    };

    for cost in cost_spec.get_modifier_specs() {
        let Some(current_val) =
            attr_set.get_current_value(&params.attribute_id_manager, cost.get_id())
        else {
            return false;
        };
        if current_val + cost.get_value() < 0.0 {
            return false;
        }
    }

    true
}

fn ability_has_any_tags(
    ability: &GameplayAbility,
    tags: &[GameplayTag],
    tag_manager: &Res<GameplayTagManager>,
) -> bool {
    let Some(ability_bits) =
        tag_bits_from_tags_with_manager(ability.get_tags().get_ability_asset_tags(), tag_manager)
    else {
        return false;
    };
    let Some(query_bits) = tag_bits_from_tags_with_manager(tags, tag_manager) else {
        return false;
    };

    ability_bits
        .iter()
        .zip(query_bits.iter())
        .any(|(a, b)| (a & b) != 0)
}
