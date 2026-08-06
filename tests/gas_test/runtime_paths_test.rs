use super::support_test::{
    ability_task_count, active_ability_count, active_effect_handles, add_modifier,
    add_tag_to_entity, apply_effect, apply_effect_with_payload, current_value, empty_effect_tags,
    give_ability, register_attribute, register_tag, run_fixed_update, spawn_attribute_set,
    test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_tools::{
    AbilityActivationContext, AbilitySystemComponent, AbilityTaskDef, AbilityTaskOnFinishedDef,
    ActiveGameplayEffects, AttributeId, AttributeSet, EffectDurationTicks, EffectPayload,
    GameplayAbility, GameplayAbilitySystemBundle, GameplayAbilitySystemSet, GameplayEffect,
    GameplayExecutionQueue, GameplayTag, GameplayTagContainer, Modifier, ModifierEvaluationContext,
    ModifierMagnitude, ModifierMagnitudeCalculation, ModifierOperation, StackingPolicy,
};
use std::sync::Arc;

#[derive(Resource)]
struct OneShotEffectRequest {
    target: Entity,
    effect: Arc<GameplayEffect>,
    submitted: bool,
}

fn submit_one_shot_effect_request(
    mut request: ResMut<OneShotEffectRequest>,
    mut execution_queue: ResMut<GameplayExecutionQueue>,
) {
    if request.submitted {
        return;
    }
    let payload = EffectPayload::new(request.target, None, 1);
    execution_queue.push_application(request.target, request.effect.clone(), payload);
    request.submitted = true;
}

struct LevelMagnitude {
    scale: f32,
}

impl ModifierMagnitudeCalculation for LevelMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        context.level() as f32 * self.scale
    }
}

struct SnapshotCurrentMagnitude {
    attribute: AttributeId,
}

impl ModifierMagnitudeCalculation for SnapshotCurrentMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get_current_value(context.attribute_id_manager(), self.attribute)
                    .ok()
                    .flatten()
            })
            .unwrap_or(0.0)
    }
}

struct SourceTagMagnitude {
    tag: GameplayTag,
    tagged: f32,
    untagged: f32,
}

impl ModifierMagnitudeCalculation for SourceTagMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        let has_tag = context
            .source_tags()
            .is_some_and(|tags| tags.has_tag(&self.tag));
        if has_tag { self.tagged } else { self.untagged }
    }
}

#[test]
fn gameplay_ability_system_bundle_is_the_explicit_composition_root() {
    let mut app = test_app();
    let tag_only = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes_only = app.world_mut().spawn(AttributeSet::default()).id();
    let complete_actor = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle::default())
        .id();

    assert!(
        app.world()
            .entity(tag_only)
            .get::<ActiveGameplayEffects>()
            .is_none()
    );
    assert!(
        app.world()
            .entity(attributes_only)
            .get::<ActiveGameplayEffects>()
            .is_none()
    );

    let actor = app.world().entity(complete_actor);
    assert!(actor.contains::<AbilitySystemComponent>());
    assert!(actor.contains::<AttributeSet>());
    assert!(actor.contains::<GameplayTagContainer>());
    assert!(actor.contains::<ActiveGameplayEffects>());
}

#[test]
fn fixed_update_processes_queued_effect_before_next_duration_tick() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, effect, EffectPayload::new(target, None, 1));

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn fixed_update_activation_tasks_and_cleanup_run_in_plugin_order() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        vec![AbilityTaskDef::wait_ticks(
            1,
            AbilityTaskOnFinishedDef::EndAbility,
        )],
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
        queue.push_activation(source, source, handle, context);
    }

    run_fixed_update(&mut app);
    assert_eq!(active_ability_count(&mut app), 1);
    assert_eq!(ability_task_count(&mut app), 1);

    run_fixed_update(&mut app);
    assert_eq!(active_ability_count(&mut app), 0);
    assert_eq!(ability_task_count(&mut app), 0);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        0
    );
}

#[test]
fn startup_instant_task_executes_during_activation_tick() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::ApplyGameplayEffectToTarget { effect },
        )],
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
        queue.push_activation(source, target, handle, context);
    }

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
    assert_eq!(ability_task_count(&mut app), 0);
}

#[test]
fn request_producer_phase_is_consumed_in_same_fixed_tick() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    app.insert_resource(OneShotEffectRequest {
        target,
        effect: Arc::new(GameplayEffect::new(
            vec![add_modifier(health, 5.0)],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        )),
        submitted: false,
    });
    app.add_systems(
        FixedUpdate,
        submit_one_shot_effect_request.in_set(GameplayAbilitySystemSet::RequestProducers),
    );

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn request_produced_after_resolver_waits_for_next_fixed_tick() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    app.insert_resource(OneShotEffectRequest {
        target,
        effect: Arc::new(GameplayEffect::new(
            vec![add_modifier(health, 5.0)],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        )),
        submitted: false,
    });
    app.add_systems(
        FixedUpdate,
        submit_one_shot_effect_request
            .after(GameplayAbilitySystemSet::GameplayResolve)
            .before(GameplayAbilitySystemSet::UpdateEffectTagRequirements),
    );

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert_eq!(app.world().resource::<GameplayExecutionQueue>().len(), 1);

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn calculated_magnitude_can_use_effect_level() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let target = spawn_attribute_set(&mut app, damage, 0.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            damage,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(LevelMagnitude { scale: 3.0 })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect_with_payload(
        &mut app,
        target,
        effect,
        EffectPayload::new(target, None, 4),
    ));
    assert_eq!(current_value(&mut app, target, damage), 12.0);
}

#[test]
fn calculated_magnitude_can_use_source_snapshot() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let damage = register_attribute(&mut app, "Damage");
    let source = spawn_attribute_set(&mut app, power, 7.0);
    let target = spawn_attribute_set(&mut app, damage, 0.0);
    let snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<AttributeSet>()
        .unwrap()
        .make_snapshot(source);
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            damage,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(SnapshotCurrentMagnitude { attribute: power })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect_with_payload(
        &mut app,
        target,
        effect,
        EffectPayload::new(source, None, 1).with_source_snapshot(snapshot),
    ));
    assert_eq!(current_value(&mut app, target, damage), 7.0);
}

#[test]
fn calculated_magnitude_can_read_source_tags() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let empowered = register_tag(&mut app, "State.Empowered");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let target = spawn_attribute_set(&mut app, damage, 0.0);
    add_tag_to_entity(&mut app, source, empowered);
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            damage,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(SourceTagMagnitude {
                tag: empowered,
                tagged: 20.0,
                untagged: 5.0,
            })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect_with_payload(
        &mut app,
        target,
        effect,
        EffectPayload::new(source, None, 1),
    ));
    assert_eq!(current_value(&mut app, target, damage), 20.0);
}

#[test]
fn stale_active_effect_handle_cannot_remove_reused_slot() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    let stale_handle = active_effect_handles(&app, target)[0];
    let removed = app
        .world_mut()
        .run_system_once(move |mut params: bevy_tools::AbilitySystemParams| {
            bevy_tools::remove_active_effect(stale_handle, &mut params)
        })
        .unwrap()
        .unwrap();
    assert!(removed);

    assert!(apply_effect(&mut app, target, target, effect));
    let replacement_handle = active_effect_handles(&app, target)[0];
    assert_eq!(stale_handle.get_slot(), replacement_handle.get_slot());
    assert_ne!(
        stale_handle.get_generation(),
        replacement_handle.get_generation()
    );

    let removed = app
        .world_mut()
        .run_system_once(move |mut params: bevy_tools::AbilitySystemParams| {
            bevy_tools::remove_active_effect(stale_handle, &mut params)
        })
        .unwrap()
        .unwrap();
    assert!(!removed);
    assert_eq!(
        active_effect_handles(&app, target),
        vec![replacement_handle]
    );
}
