use super::support_test::{
    ability_task_count, attribute_set, current_value, empty_effect_tags, give_ability,
    instant_add_effect, modifier, register_attribute, run_ability_tasks,
    run_gameplay_execution_queue, spawn_ability_task, spawn_active_ability, spawn_attribute_set,
    test_app,
};
use bevy::prelude::*;
use bevy_tools::{
    AbilityActivationContext, AbilityActivationReason, AbilityActivationRequest,
    AbilityActivationStatus, AbilityChainContext, AbilitySpecHandle, AbilitySystemComponent,
    AbilityTask, AbilityTaskDef, AbilityTaskEvent, AbilityTaskExecutionContext,
    AbilityTaskOnFinished, AbilityTaskOnFinishedDef, ActiveGameplayAbility, AttributeId,
    EffectDurationTicks, EffectPayload, GameplayAbility, GameplayEffect, GameplayExecutionQueue,
    Modifier, ModifierEvaluationContext, ModifierMagnitude, ModifierMagnitudeCalculation,
    ModifierOperation, StackingPolicy, UniqueName,
};
use std::sync::Arc;

struct QueuedEffectContextMagnitude {
    expected_causer: Entity,
    snapshot_attribute: AttributeId,
}

impl ModifierMagnitudeCalculation for QueuedEffectContextMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        if context.causer() != Some(self.expected_causer) {
            return 0.0;
        }

        context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get_current_value(context.attribute_id_manager(), self.snapshot_attribute)
                    .ok()
                    .flatten()
            })
            .unwrap_or(0.0)
    }
}

#[derive(Resource, Default)]
struct CapturedAbilityTaskEvent {
    source: Option<Entity>,
    target: Option<Entity>,
    active_ability: Option<Entity>,
    spec_handle: Option<AbilitySpecHandle>,
    event_id: Option<UniqueName>,
    level: Option<u32>,
}

fn capture_ability_task_event(
    event: On<AbilityTaskEvent>,
    mut captured: ResMut<CapturedAbilityTaskEvent>,
) {
    captured.source = Some(event.get_source());
    captured.target = Some(event.get_target());
    captured.active_ability = Some(event.get_active_ability());
    captured.spec_handle = Some(event.get_spec_handle());
    captured.event_id = Some(event.get_event_id());
    captured.level = Some(event.get_level());
}

#[test]
fn gameplay_queue_processes_entire_effect_batch() {
    const REQUEST_COUNT: usize = 257;

    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 0.0);
    let effect = instant_add_effect(health, 1.0);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        for _ in 0..REQUEST_COUNT {
            queue.push_application(target, effect.clone(), EffectPayload::new(target, None, 1));
        }
    }

    run_gameplay_execution_queue(&mut app);
    assert_eq!(
        current_value(&mut app, target, health),
        REQUEST_COUNT as f32
    );
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn gameplay_queue_processes_entire_activation_batch() {
    const REQUEST_COUNT: usize = 129;

    let mut app = test_app();
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        true,
    ));
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let handle = give_ability(&mut app, source, ability);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        for _ in 0..REQUEST_COUNT {
            let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
            queue.push_activation(source, source, handle, context);
        }
    }

    run_gameplay_execution_queue(&mut app);
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn ability_activation_request_is_preserved_through_startup() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = app.world_mut().spawn_empty().id();
    let instigator = app.world_mut().spawn_empty().id();
    let causer = app.world_mut().spawn_empty().id();
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let chain = AbilityChainContext::root(handle, 42);
    let context = AbilityActivationContext::direct(source, chain.clone())
        .with_instigator(instigator)
        .with_causer(Some(causer));

    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push(AbilityActivationRequest::new(
            source, target, handle, context,
        ));
    run_gameplay_execution_queue(&mut app);

    let world = app.world_mut();
    let mut query = world.query::<&ActiveGameplayAbility>();
    let active_ability = query.single(world).unwrap();
    assert_eq!(active_ability.get_source(), source);
    assert_eq!(active_ability.get_target(), target);
    assert_eq!(active_ability.get_spec_handle(), handle);
    assert_eq!(
        active_ability.get_activation_context().get_instigator(),
        instigator
    );
    assert_eq!(
        active_ability.get_activation_context().get_causer(),
        Some(causer)
    );
    assert_eq!(
        active_ability.get_activation_context().get_reason(),
        AbilityActivationReason::Direct
    );
    assert_eq!(active_ability.get_chain(), Some(&chain));
}

#[test]
fn startup_task_context_preserves_ability_handle_and_level() {
    let mut app = test_app();
    app.init_resource::<CapturedAbilityTaskEvent>();
    app.world_mut().add_observer(capture_ability_task_event);

    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = app.world_mut().spawn_empty().id();
    let event_id = app
        .world_mut()
        .resource_mut::<bevy_tools::UniqueNamePool>()
        .new_name("Ability.Event.StartupContext")
        .unwrap();
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        vec![AbilityTaskDef::instant(
            AbilityTaskOnFinishedDef::EmitEvent { event_id },
        )],
        None,
        None,
        Vec::new(),
        false,
        false,
    ));
    let handle = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<AbilitySystemComponent>()
        .unwrap()
        .give_ability(ability, 9, None);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
        queue.push_activation(source, target, handle, context);
    }
    run_gameplay_execution_queue(&mut app);

    let captured = app.world().resource::<CapturedAbilityTaskEvent>();
    assert_eq!(captured.source, Some(source));
    assert_eq!(captured.target, Some(target));
    assert_eq!(captured.spec_handle, Some(handle));
    assert_eq!(captured.event_id, Some(event_id));
    assert_eq!(captured.level, Some(9));
}

#[test]
fn gameplay_queue_processes_effect_requests_fifo() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 0.0);
    let first = Arc::new(GameplayEffect::new(
        vec![modifier(health, ModifierOperation::Override, 1.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let second = Arc::new(GameplayEffect::new(
        vec![modifier(health, ModifierOperation::Override, 2.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, first, EffectPayload::new(target, None, 1));
        queue.push_application(target, second, EffectPayload::new(target, None, 1));
    }

    run_gameplay_execution_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 2.0);
}

#[test]
fn gameplay_queue_processes_activation_requests_fifo() {
    let mut app = test_app();
    let marker = register_attribute(&mut app, "Marker");
    let attributes = attribute_set(&app, marker, 0.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let first_effect = Arc::new(GameplayEffect::new(
        vec![modifier(marker, ModifierOperation::Override, 1.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let second_effect = Arc::new(GameplayEffect::new(
        vec![modifier(marker, ModifierOperation::Override, 2.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let first = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![first_effect],
        true,
        false,
    ));
    let second = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![second_effect],
        true,
        false,
    ));
    let first_handle = give_ability(&mut app, source, first);
    let second_handle = give_ability(&mut app, source, second);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let first_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(first_handle));
        let second_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(second_handle));
        queue.push_activation(source, source, first_handle, first_context);
        queue.push_activation(source, source, second_handle, second_context);
    }

    run_gameplay_execution_queue(&mut app);
    assert_eq!(current_value(&mut app, source, marker), 2.0);
}

#[test]
fn gameplay_execution_queue_preserves_cross_type_fifo() {
    let mut app = test_app();
    let marker = register_attribute(&mut app, "Marker");
    let attributes = attribute_set(&app, marker, 0.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let activation_effect = Arc::new(GameplayEffect::new(
        vec![modifier(marker, ModifierOperation::Override, 2.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let queued_effect = Arc::new(GameplayEffect::new(
        vec![modifier(marker, ModifierOperation::Override, 1.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![activation_effect],
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        let context = AbilityActivationContext::direct(source, queue.new_root_chain(handle));
        queue.push_activation(source, source, handle, context);
        queue.push_application(source, queued_effect, EffectPayload::new(source, None, 1));
    }

    run_gameplay_execution_queue(&mut app);
    assert_eq!(current_value(&mut app, source, marker), 1.0);
}

#[test]
fn task_without_active_ability_is_removed() {
    let mut app = test_app();
    let missing_active = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(missing_active).despawn();
    let context = AbilityTaskExecutionContext::new(
        missing_active,
        missing_active,
        AbilitySpecHandle::new(0),
        1,
    );
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(missing_active, context, AbilityTaskOnFinished::None),
    );

    assert_eq!(ability_task_count(&mut app), 1);
    run_ability_tasks(&mut app);
    assert_eq!(ability_task_count(&mut app), 0);
}

#[test]
fn task_can_enqueue_gameplay_effect_application() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source = app.world_mut().spawn_empty().id();
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let active = spawn_active_ability(&mut app, source, target, AbilitySpecHandle::new(123));
    let context = AbilityTaskExecutionContext::new(source, target, AbilitySpecHandle::new(123), 1);
    let effect = instant_add_effect(health, 5.0);
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            context,
            AbilityTaskOnFinished::ApplyGameplayEffect { effect },
        ),
    );

    run_ability_tasks(&mut app);
    assert_eq!(app.world().resource::<GameplayExecutionQueue>().len(), 1);
    assert_eq!(current_value(&mut app, target, health), 10.0);

    run_gameplay_execution_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
}

#[test]
fn task_effect_application_inherits_activation_context_payload() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let damage = register_attribute(&mut app, "Damage");
    let causer = app.world_mut().spawn_empty().id();
    let source = spawn_attribute_set(&mut app, power, 11.0);
    let target = spawn_attribute_set(&mut app, damage, 0.0);
    let snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<bevy_tools::AttributeSet>()
        .unwrap()
        .make_snapshot(source);
    let handle = AbilitySpecHandle::new(456);
    let context = AbilityActivationContext::direct(source, AbilityChainContext::root(handle, 0))
        .with_causer(Some(causer))
        .with_source_snapshot(snapshot);
    let active = app
        .world_mut()
        .spawn(ActiveGameplayAbility::new(
            source,
            handle,
            target,
            AbilityActivationStatus::Active,
            context,
        ))
        .id();
    let task_context = AbilityTaskExecutionContext::new(source, target, handle, 1);
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            damage,
            ModifierOperation::Add,
            ModifierMagnitude::Calculated(Box::new(QueuedEffectContextMagnitude {
                expected_causer: causer,
                snapshot_attribute: power,
            })),
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            task_context,
            AbilityTaskOnFinished::ApplyGameplayEffect { effect },
        ),
    );

    run_ability_tasks(&mut app);
    run_gameplay_execution_queue(&mut app);

    assert_eq!(current_value(&mut app, target, damage), 11.0);
}

#[test]
fn task_can_enqueue_ability_activation() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let target = source;
    let ability = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);
    let active = spawn_active_ability(&mut app, source, target, AbilitySpecHandle::new(321));
    let context = AbilityTaskExecutionContext::new(source, target, AbilitySpecHandle::new(321), 1);
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            context,
            AbilityTaskOnFinished::ActivateAbility { handle },
        ),
    );

    run_ability_tasks(&mut app);
    assert_eq!(app.world().resource::<GameplayExecutionQueue>().len(), 1);

    run_gameplay_execution_queue(&mut app);
    assert!(app.world().resource::<GameplayExecutionQueue>().is_empty());
}

#[test]
fn task_emit_event_triggers_observer_with_full_payload() {
    let mut app = test_app();
    app.init_resource::<CapturedAbilityTaskEvent>();
    app.world_mut().add_observer(capture_ability_task_event);

    let source = app.world_mut().spawn_empty().id();
    let target = app.world_mut().spawn_empty().id();
    let handle = AbilitySpecHandle::new(77);
    let event_id = app
        .world_mut()
        .resource_mut::<bevy_tools::UniqueNamePool>()
        .new_name("Ability.Event.ComboWindow")
        .unwrap();
    let active = spawn_active_ability(&mut app, source, target, handle);
    let context = AbilityTaskExecutionContext::new(source, target, handle, 9);
    let task = AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EmitEvent { event_id })
        .instantiate(active, context);
    spawn_ability_task(&mut app, task);

    run_ability_tasks(&mut app);

    let captured = app.world().resource::<CapturedAbilityTaskEvent>();
    assert_eq!(captured.source, Some(source));
    assert_eq!(captured.target, Some(target));
    assert_eq!(captured.active_ability, Some(active));
    assert_eq!(captured.spec_handle, Some(handle));
    assert_eq!(captured.event_id, Some(event_id));
    assert_eq!(captured.level, Some(9));
}
