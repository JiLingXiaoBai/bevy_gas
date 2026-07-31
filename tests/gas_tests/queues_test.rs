use super::common_test::{
    ability_task_count, current_value, instant_add_effect, register_attribute,
    run_ability_activation_queue, run_ability_tasks, run_effect_application_queue,
    set_ability_queue_limit, set_effect_queue_limit, spawn_ability_task, test_app,
};
use bevy::prelude::*;
use bevy_tools::{
    AbilityActivationContext, AbilityActivationQueue, AbilityActivationStatus, AbilityChainContext,
    AbilitySpecHandle, AbilitySystemComponent, AbilityTask, AbilityTaskDef, AbilityTaskEvent,
    AbilityTaskOnFinished, AbilityTaskOnFinishedDef, ActiveGameplayAbility, AttributeId,
    EffectContext, EffectDurationTicks, EffectPayload, GameplayAbility, GameplayEffect,
    GameplayEffectApplicationQueue, Modifier, ModifierMagnitude, ModifierMagnitudeCalculation,
    ModifierOperation, StackingPolicy, UniqueName,
};
use std::sync::Arc;

struct QueuedEffectContextMagnitude {
    expected_causer: Entity,
    snapshot_attribute: AttributeId,
}

impl ModifierMagnitudeCalculation for QueuedEffectContextMagnitude {
    fn calculate(&self, context: &EffectContext) -> f32 {
        if context.causer() != Some(self.expected_causer) {
            return 0.0;
        }

        context
            .source_snapshot()
            .and_then(|snapshot| {
                snapshot.get_current_value(context.attribute_id_manager(), self.snapshot_attribute)
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
fn effect_application_queue_respects_per_tick_limit() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = super::common_test::spawn_attribute_set(&mut app, health, 0.0);
    let effect = instant_add_effect(health, 1.0);
    set_effect_queue_limit(&mut app, 1);

    {
        let mut queue = app
            .world_mut()
            .resource_mut::<GameplayEffectApplicationQueue>();
        queue.push_application(target, effect.clone(), EffectPayload::new(target, None, 1));
        queue.push_application(target, effect, EffectPayload::new(target, None, 1));
    }

    run_effect_application_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 1.0);
    assert_eq!(
        app.world()
            .resource::<GameplayEffectApplicationQueue>()
            .len(),
        1
    );

    run_effect_application_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 2.0);
    assert!(
        app.world()
            .resource::<GameplayEffectApplicationQueue>()
            .is_empty()
    );
}

#[test]
fn ability_activation_queue_respects_per_tick_limit() {
    let mut app = test_app();
    let first = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let second = Arc::new(GameplayAbility::new(
        bevy_tools::AbilityTags::default(),
        Vec::new(),
        None,
        None,
        Vec::new(),
        true,
        false,
    ));
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let first_handle = super::common_test::give_ability(&mut app, source, first);
    let second_handle = super::common_test::give_ability(&mut app, source, second);
    set_ability_queue_limit(&mut app, 1);

    {
        let mut queue = app.world_mut().resource_mut::<AbilityActivationQueue>();
        let first_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(first_handle));
        let second_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(second_handle));
        queue.push_activation(source, source, first_handle, first_context);
        queue.push_activation(source, source, second_handle, second_context);
    }

    run_ability_activation_queue(&mut app);
    assert_eq!(app.world().resource::<AbilityActivationQueue>().len(), 1);

    run_ability_activation_queue(&mut app);
    assert!(app.world().resource::<AbilityActivationQueue>().is_empty());
}

#[test]
fn queue_limits_clamp_zero_to_one() {
    let mut app = test_app();

    set_effect_queue_limit(&mut app, 0);
    set_ability_queue_limit(&mut app, 0);

    assert_eq!(
        app.world()
            .resource::<GameplayEffectApplicationQueue>()
            .max_applications_per_tick(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<AbilityActivationQueue>()
            .max_activations_per_tick(),
        1
    );
}

#[test]
fn effect_application_queue_processes_requests_fifo() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = super::common_test::spawn_attribute_set(&mut app, health, 0.0);
    let first = Arc::new(GameplayEffect::new(
        vec![super::common_test::modifier(
            health,
            ModifierOperation::Override,
            1.0,
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        super::common_test::empty_effect_tags(),
    ));
    let second = Arc::new(GameplayEffect::new(
        vec![super::common_test::modifier(
            health,
            ModifierOperation::Override,
            2.0,
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        super::common_test::empty_effect_tags(),
    ));

    {
        let mut queue = app
            .world_mut()
            .resource_mut::<GameplayEffectApplicationQueue>();
        queue.push_application(target, first, EffectPayload::new(target, None, 1));
        queue.push_application(target, second, EffectPayload::new(target, None, 1));
    }

    run_effect_application_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 2.0);
}

#[test]
fn ability_activation_queue_processes_requests_fifo() {
    let mut app = test_app();
    let marker = register_attribute(&mut app, "Marker");
    let attributes = super::common_test::attribute_set(&app, marker, 0.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let first_effect = Arc::new(GameplayEffect::new(
        vec![super::common_test::modifier(
            marker,
            ModifierOperation::Override,
            1.0,
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        super::common_test::empty_effect_tags(),
    ));
    let second_effect = Arc::new(GameplayEffect::new(
        vec![super::common_test::modifier(
            marker,
            ModifierOperation::Override,
            2.0,
        )],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        super::common_test::empty_effect_tags(),
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
    let first_handle = super::common_test::give_ability(&mut app, source, first);
    let second_handle = super::common_test::give_ability(&mut app, source, second);

    {
        let mut queue = app.world_mut().resource_mut::<AbilityActivationQueue>();
        let first_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(first_handle));
        let second_context =
            AbilityActivationContext::direct(source, queue.new_root_chain(second_handle));
        queue.push_activation(source, source, first_handle, first_context);
        queue.push_activation(source, source, second_handle, second_context);
    }

    run_ability_activation_queue(&mut app);
    assert_eq!(current_value(&mut app, source, marker), 2.0);
}

#[test]
fn task_without_active_ability_is_removed() {
    let mut app = test_app();
    let missing_active = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(missing_active).despawn();
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(missing_active, AbilityTaskOnFinished::None),
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
    let target = super::common_test::spawn_attribute_set(&mut app, health, 10.0);
    let active = super::common_test::spawn_active_ability(
        &mut app,
        source,
        target,
        AbilitySpecHandle::new(123),
    );
    let effect = instant_add_effect(health, 5.0);
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            AbilityTaskOnFinished::ApplyGameplayEffect {
                source,
                target,
                effect,
                level: 1,
            },
        ),
    );

    run_ability_tasks(&mut app);
    assert_eq!(
        app.world()
            .resource::<GameplayEffectApplicationQueue>()
            .len(),
        1
    );
    assert_eq!(current_value(&mut app, target, health), 10.0);

    run_effect_application_queue(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
}

#[test]
fn task_effect_application_inherits_activation_context_payload() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let damage = register_attribute(&mut app, "Damage");
    let causer = app.world_mut().spawn_empty().id();
    let source = super::common_test::spawn_attribute_set(&mut app, power, 11.0);
    let target = super::common_test::spawn_attribute_set(&mut app, damage, 0.0);
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
        super::common_test::empty_effect_tags(),
    ));

    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            AbilityTaskOnFinished::ApplyGameplayEffect {
                source,
                target,
                effect,
                level: 1,
            },
        ),
    );

    run_ability_tasks(&mut app);
    run_effect_application_queue(&mut app);

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
    let handle = super::common_test::give_ability(&mut app, source, ability);
    let active = super::common_test::spawn_active_ability(
        &mut app,
        source,
        target,
        AbilitySpecHandle::new(321),
    );
    spawn_ability_task(
        &mut app,
        AbilityTask::instant(
            active,
            AbilityTaskOnFinished::ActivateAbility {
                source,
                target,
                handle,
            },
        ),
    );

    run_ability_tasks(&mut app);
    assert_eq!(app.world().resource::<AbilityActivationQueue>().len(), 1);

    run_ability_activation_queue(&mut app);
    assert!(app.world().resource::<AbilityActivationQueue>().is_empty());
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
        .new_name("Ability.Event.ComboWindow");
    let active = super::common_test::spawn_active_ability(&mut app, source, target, handle);
    let task = AbilityTaskDef::instant(AbilityTaskOnFinishedDef::EmitEvent { event_id })
        .instantiate(active, source, target, handle, 9);
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
