use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_tools::attributes::{
    AttributeId, AttributeIdManager, AttributeIdRegister, AttributeRegion, AttributeSet,
};
use bevy_tools::gameplay_abilities::{AbilitySpecHandle, AbilityTask, ActiveGameplayAbility};
use bevy_tools::gameplay_effects::{
    EffectDurationTicks, EffectPayload, EffectTags, GameplayEffect, TagRequirements,
};
use bevy_tools::gameplay_tags::{
    GameplayTag, GameplayTagContainer, GameplayTagManager, GameplayTagRegister,
};
use bevy_tools::modifiers::{Modifier, ModifierMagnitude, ModifierOperation};
use bevy_tools::{
    AbilityActivationContext, AbilityChainContext, AbilitySystemComponent, AbilitySystemParams,
    ActiveGameplayEffectTargetIndex, GameplayAbilitySystemPlugin, apply_gameplay_effect,
    cleanup_finished_abilities_system, process_ability_activation_queue_system,
    process_gameplay_effect_application_queue_system, reconcile_active_effect_target_index_system,
    tick_ability_tasks_system, tick_effect_duration_system, tick_effect_period_system,
    try_activate_ability_by_handle, update_active_effect_tag_requirements_system,
};
use std::sync::Arc;

pub fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayAbilitySystemPlugin));
    app
}

pub fn register_tag(app: &mut App, name: &str) -> GameplayTag {
    let name = name.to_string();
    app.world_mut()
        .run_system_once(move |mut register: GameplayTagRegister| {
            register.request_or_register_tag(&name).unwrap()
        })
        .unwrap()
}

pub fn register_attribute(app: &mut App, name: &str) -> AttributeId {
    let name = name.to_string();
    app.world_mut()
        .run_system_once(move |mut register: AttributeIdRegister| {
            register
                .request_or_register_attribute_id(&name, AttributeRegion::Cold)
                .unwrap()
        })
        .unwrap()
}

pub fn register_hot_attribute(app: &mut App, name: &str) -> AttributeId {
    let name = name.to_string();
    app.world_mut()
        .run_system_once(move |mut register: AttributeIdRegister| {
            register
                .request_or_register_attribute_id(&name, AttributeRegion::Hot)
                .unwrap()
        })
        .unwrap()
}

pub fn empty_effect_tags() -> EffectTags {
    effect_tags(Vec::new(), Vec::new())
}

pub fn effect_tags(asset_tags: Vec<GameplayTag>, granted_tags: Vec<GameplayTag>) -> EffectTags {
    EffectTags::new(
        asset_tags,
        granted_tags,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        Vec::new(),
        Vec::new(),
    )
}

pub fn add_modifier(attribute: AttributeId, value: f32) -> Modifier {
    modifier(attribute, ModifierOperation::Add, value)
}

pub fn modifier(attribute: AttributeId, operation: ModifierOperation, value: f32) -> Modifier {
    Modifier::new(attribute, operation, ModifierMagnitude::Flat(value))
}

pub fn attribute_set(app: &App, attribute: AttributeId, base_value: f32) -> AttributeSet {
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(
            app.world().resource::<AttributeIdManager>(),
            attribute,
            base_value,
            None,
        )
        .unwrap();
    attributes
}

pub fn spawn_attribute_set(app: &mut App, attribute: AttributeId, base_value: f32) -> Entity {
    let attributes = attribute_set(app, attribute, base_value);
    app.world_mut().spawn(attributes).id()
}

pub fn instant_add_effect(attribute: AttributeId, value: f32) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        vec![add_modifier(attribute, value)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        bevy_tools::StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ))
}

pub fn add_tag_to_entity(app: &mut App, entity: Entity, tag: GameplayTag) {
    app.world_mut()
        .run_system_once(
            move |mut query: Query<&mut GameplayTagContainer>,
                  tag_manager: Res<GameplayTagManager>| {
                query
                    .get_mut(entity)
                    .unwrap()
                    .add_tag(&tag, &tag_manager)
                    .unwrap();
            },
        )
        .unwrap();
}

pub fn remove_tag_from_entity(app: &mut App, entity: Entity, tag: GameplayTag) {
    app.world_mut()
        .run_system_once(
            move |mut query: Query<&mut GameplayTagContainer>,
                  tag_manager: Res<GameplayTagManager>| {
                query
                    .get_mut(entity)
                    .unwrap()
                    .remove_tag(&tag, &tag_manager)
                    .unwrap();
            },
        )
        .unwrap();
}

pub fn apply_effect(
    app: &mut App,
    target: Entity,
    source: Entity,
    effect: Arc<GameplayEffect>,
) -> bool {
    apply_effect_result(app, target, source, effect).is_ok()
}

pub fn apply_effect_result(
    app: &mut App,
    target: Entity,
    source: Entity,
    effect: Arc<GameplayEffect>,
) -> Result<(), bevy_tools::GameplayEffectApplicationError> {
    apply_effect_with_payload_result(app, target, effect, EffectPayload::new(source, None, 1))
}

pub fn apply_effect_with_payload(
    app: &mut App,
    target: Entity,
    effect: Arc<GameplayEffect>,
    payload: EffectPayload,
) -> bool {
    apply_effect_with_payload_result(app, target, effect, payload).is_ok()
}

pub fn apply_effect_with_payload_result(
    app: &mut App,
    target: Entity,
    effect: Arc<GameplayEffect>,
    payload: EffectPayload,
) -> Result<(), bevy_tools::GameplayEffectApplicationError> {
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            apply_gameplay_effect(target, &effect, &mut params, &payload)
        })
        .unwrap()
}

pub fn activate_ability(
    app: &mut App,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) -> bool {
    activate_ability_result(app, source, target, handle).is_ok()
}

pub fn activate_ability_result(
    app: &mut App,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) -> Result<(), bevy_tools::AbilityActivationError> {
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            try_activate_ability_by_handle(
                source,
                target,
                handle,
                AbilityActivationContext::direct(source, AbilityChainContext::root(handle, 0)),
                &mut params,
            )
        })
        .unwrap()
}

pub fn activate_ability_with_context(
    app: &mut App,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
    context: AbilityActivationContext,
) -> Result<(), bevy_tools::AbilityActivationError> {
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            try_activate_ability_by_handle(source, target, handle, context.clone(), &mut params)
        })
        .unwrap()
}

pub fn current_value(app: &mut App, entity: Entity, attribute: AttributeId) -> f32 {
    let manager = app.world().resource::<AttributeIdManager>().clone();
    app.world_mut()
        .entity_mut(entity)
        .get_mut::<AttributeSet>()
        .unwrap()
        .get_current_value(&manager, attribute)
        .unwrap()
        .unwrap()
}

pub fn active_effect_handles(app: &App, target: Entity) -> Vec<Entity> {
    app.world()
        .resource::<ActiveGameplayEffectTargetIndex>()
        .handles_for(target)
        .to_vec()
}

pub fn give_ability(
    app: &mut App,
    owner: Entity,
    ability: Arc<bevy_tools::GameplayAbility>,
) -> AbilitySpecHandle {
    app.world_mut()
        .entity_mut(owner)
        .get_mut::<AbilitySystemComponent>()
        .unwrap()
        .give_ability(ability, 1, None)
}

pub fn run_effect_duration_tick(app: &mut App) {
    app.world_mut()
        .run_system_once(tick_effect_duration_system)
        .unwrap();
}

pub fn run_effect_period_tick(app: &mut App) {
    app.world_mut()
        .run_system_once(tick_effect_period_system)
        .unwrap();
}

pub fn run_effect_tag_requirements_update(app: &mut App) {
    app.world_mut()
        .run_system_once(update_active_effect_tag_requirements_system)
        .unwrap();
}

pub fn run_ability_tasks(app: &mut App) {
    app.world_mut()
        .run_system_once(tick_ability_tasks_system)
        .unwrap();
}

pub fn run_ability_activation_queue(app: &mut App) {
    app.world_mut()
        .run_system_once(process_ability_activation_queue_system)
        .unwrap();
}

pub fn run_effect_application_queue(app: &mut App) {
    app.world_mut()
        .run_system_once(process_gameplay_effect_application_queue_system)
        .unwrap();
}

pub fn run_finished_ability_cleanup(app: &mut App) {
    app.world_mut()
        .run_system_once(cleanup_finished_abilities_system)
        .unwrap();
}

pub fn run_active_effect_index_reconcile(app: &mut App) {
    app.world_mut()
        .run_system_once(reconcile_active_effect_target_index_system)
        .unwrap();
}

pub fn run_fixed_update(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

pub fn spawn_active_ability(
    app: &mut App,
    source: Entity,
    target: Entity,
    handle: AbilitySpecHandle,
) -> Entity {
    app.world_mut()
        .spawn(ActiveGameplayAbility::new(
            source,
            handle,
            target,
            bevy_tools::AbilityActivationStatus::Active,
            AbilityActivationContext::direct(source, AbilityChainContext::root(handle, 0)),
        ))
        .id()
}

pub fn spawn_ability_task(app: &mut App, task: AbilityTask) -> Entity {
    app.world_mut().spawn(task).id()
}

pub fn active_ability_count(app: &mut App) -> usize {
    app.world_mut()
        .run_system_once(|query: Query<&ActiveGameplayAbility>| query.iter().count())
        .unwrap()
}

pub fn ability_task_count(app: &mut App) -> usize {
    app.world_mut()
        .run_system_once(|query: Query<&AbilityTask>| query.iter().count())
        .unwrap()
}

pub fn active_ability_entity_for_spec(app: &mut App, handle: AbilitySpecHandle) -> Option<Entity> {
    app.world_mut()
        .run_system_once(move |query: Query<(Entity, &ActiveGameplayAbility)>| {
            query.iter().find_map(|(entity, ability)| {
                (ability.get_spec_handle() == handle).then_some(entity)
            })
        })
        .unwrap()
}

pub fn active_ability_context_for_spec(
    app: &mut App,
    handle: AbilitySpecHandle,
) -> Option<AbilityActivationContext> {
    app.world_mut()
        .run_system_once(move |query: Query<&ActiveGameplayAbility>| {
            query
                .iter()
                .find(|ability| ability.get_spec_handle() == handle)
                .map(|ability| ability.get_activation_context().clone())
        })
        .unwrap()
}
