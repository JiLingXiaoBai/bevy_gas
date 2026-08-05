use super::common_test::{
    active_effect_handles, add_modifier, add_tag_to_entity, apply_effect, apply_effect_result,
    attribute_set, current_value, effect_tags, empty_effect_tags, register_attribute, register_tag,
    remove_tag_from_entity, run_effect_duration_tick, run_effect_period_tick,
    run_effect_tag_requirements_update, run_fixed_update, spawn_attribute_set, test_app,
};
use bevy::ecs::system::RunSystemOnce;
use bevy_tools::{
    AbilitySystemParams, ActiveGameplayEffects, AttributeSet, EffectDurationTicks, EffectPayload,
    EffectPeriodTicks, EffectTags, GameplayEffect, GameplayEffectApplicationError,
    GameplayEffectImmunityQuery, GameplayExecutionQueue, GameplayTag, GameplayTagContainer,
    ModifierMagnitude, StackDurationPolicy, StackExpirationPolicy, StackMagnitudePolicy,
    StackOverflowPolicy, StackPeriodPolicy, StackingPolicy, StackingType, TagRequirements,
    execute_gameplay_effect_plan, prepare_gameplay_effect,
};
use std::sync::Arc;

fn tags_with_requirements(
    granted_tags: Vec<GameplayTag>,
    target_ongoing_tags: TagRequirements,
    target_removal_tags: TagRequirements,
) -> EffectTags {
    EffectTags::new(
        Vec::new(),
        granted_tags,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        target_ongoing_tags,
        TagRequirements::default(),
        target_removal_tags,
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn periodic_effect_executes_on_application_and_each_period() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 2.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(2.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 12.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 12.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 14.0);
}

#[test]
fn tag_only_periodic_effect_does_not_require_attribute_set() {
    let mut app = test_app();
    let active_tag = register_tag(&mut app, "State.Periodic");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(1.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![active_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    run_effect_period_tick(&mut app);

    assert_eq!(active_effect_handles(&app, target).len(), 1);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .is_some_and(|tags| tags.has_tag(&active_tag))
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn linear_stacking_respects_stack_limit() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 2),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert_eq!(current_value(&mut app, target, power), 15.0);
    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(!apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 20.0);

    let handles = active_effect_handles(&app, target);
    assert_eq!(handles.len(), 1);
    let active_effect = app
        .world()
        .entity(target)
        .get::<ActiveGameplayEffects>()
        .and_then(|effects| effects.get(handles[0]))
        .unwrap();
    assert_eq!(active_effect.get_stack_count(), 2);
}

#[test]
fn tag_only_stack_and_single_stack_expiration_do_not_require_attribute_set() {
    let mut app = test_app();
    let stacked_tag = register_tag(&mut app, "State.Stacked");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        2,
        StackMagnitudePolicy::None,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::KeepCurrentTick,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        stacking,
        effect_tags(Vec::new(), vec![stacked_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    let handle = active_effect_handles(&app, target)[0];
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .map(|effect| effect.get_stack_count()),
        Some(2)
    );

    run_effect_duration_tick(&mut app);

    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .map(|effect| effect.get_stack_count()),
        Some(1)
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn queued_applications_stack_against_same_batch_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ));
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, effect.clone(), EffectPayload::new(target, None, 1));
        queue.push_application(target, effect, EffectPayload::new(target, None, 1));
    }

    run_fixed_update(&mut app);
    let handles = active_effect_handles(&app, target);
    assert_eq!(handles.len(), 1);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handles[0]))
            .unwrap()
            .get_stack_count(),
        2
    );
}

#[test]
fn remove_single_stack_expiration_decrements_stack_before_removal() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        3,
        StackMagnitudePolicy::Linear,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::ResetOnSuccessfulStack,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, armor), 20.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    let handle = active_effect_handles(&app, target)[0];
    assert_eq!(
        app.world()
            .entity(target)
            .get::<ActiveGameplayEffects>()
            .and_then(|effects| effects.get(handle))
            .unwrap()
            .get_stack_count(),
        1
    );

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn remove_effects_with_tags_cleans_existing_effect_before_new_application() {
    let mut app = test_app();
    let damage = register_attribute(&mut app, "Damage");
    let buff_tag = register_tag(&mut app, "Effect.Buff.Power");
    let target = spawn_attribute_set(&mut app, damage, 10.0);
    let old_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(damage, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![buff_tag], Vec::new()),
    ));
    let replacing_effect_tags = EffectTags::new(
        Vec::new(),
        Vec::new(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        Vec::new(),
        vec![buff_tag],
    );
    let replacing_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(damage, 1.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        replacing_effect_tags,
    ));

    assert!(apply_effect(&mut app, target, target, old_effect));
    assert_eq!(current_value(&mut app, target, damage), 20.0);

    assert!(apply_effect(&mut app, target, target, replacing_effect));
    assert_eq!(current_value(&mut app, target, damage), 11.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn tag_only_instant_cleanse_does_not_require_attribute_set() {
    let mut app = test_app();
    let effect_tag = register_tag(&mut app, "Effect.TagOnly");
    let granted_tag = register_tag(&mut app, "State.TagOnly");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let active_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![effect_tag], vec![granted_tag]),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            vec![effect_tag],
        ),
    ));

    assert!(apply_effect(&mut app, target, target, active_effect));
    assert!(apply_effect(&mut app, target, target, cleanse));

    assert!(active_effect_handles(&app, target).is_empty());
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .is_some_and(|tags| !tags.has_tag(&granted_tag))
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn queued_effect_can_remove_effect_created_earlier_in_same_batch() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let buff_tag = register_tag(&mut app, "Effect.Buff.Power");
    let granted_tag = register_tag(&mut app, "State.Buffed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![buff_tag], vec![granted_tag]),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            vec![buff_tag],
        ),
    ));
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, buff, EffectPayload::new(target, None, 1));
        queue.push_application(target, cleanse, EffectPayload::new(target, None, 1));
    }

    run_fixed_update(&mut app);
    assert!(active_effect_handles(&app, target).is_empty());
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted_tag)
    );
}

#[test]
fn probability_zero_blocks_application_and_one_allows_it() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let blocked = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        0.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let allowed = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(!apply_effect(&mut app, target, target, blocked));
    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert!(apply_effect(&mut app, target, target, allowed));
    assert_eq!(current_value(&mut app, target, health), 20.0);
}

#[test]
fn invalid_probability_returns_a_specific_application_error() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 10.0)],
        EffectDurationTicks::Instant,
        None,
        f32::NAN,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(matches!(
        apply_effect_result(&mut app, target, target, effect),
        Err(GameplayEffectApplicationError::InvalidProbability { probability })
            if probability.is_nan()
    ));
}

#[test]
fn execution_preflight_preserves_effects_when_target_state_changed() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let removable = register_tag(&mut app, "Effect.Removable");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let existing = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![removable], Vec::new()),
    ));
    assert!(apply_effect(&mut app, target, target, existing));
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            vec![removable],
        ),
    ));
    let payload = EffectPayload::new(target, None, 1);
    let plan = app
        .world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            prepare_gameplay_effect(target, &effect, &mut params, &payload)
        })
        .unwrap()
        .unwrap();

    app.world_mut().entity_mut(target).remove::<AttributeSet>();
    let mut plan = Some(plan);
    let result = app
        .world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            execute_gameplay_effect_plan(plan.take().unwrap(), &mut params)
        })
        .unwrap();

    assert_eq!(
        result,
        Err(GameplayEffectApplicationError::MissingAttributeSet { target })
    );
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn non_positive_or_nan_duration_ticks_reject_application() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);

    for duration in [0.0, -1.0, f32::NAN] {
        let effect = Arc::new(GameplayEffect::new(
            vec![add_modifier(health, 10.0)],
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(duration)),
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        ));
        assert!(!apply_effect(&mut app, target, target, effect));
    }

    assert_eq!(current_value(&mut app, target, health), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn zero_tick_period_behaves_like_duration_modifier_without_period_ticks() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), true)),
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, health), 15.0);

    run_effect_period_tick(&mut app);
    assert_eq!(current_value(&mut app, target, health), 15.0);
}

#[test]
fn zero_tick_period_updates_linear_stack_magnitude() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn zero_tick_period_restores_modifier_after_inhibition() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.Enabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    add_tag_to_entity(&mut app, target, enabled);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 15.0);

    remove_tag_from_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);

    add_tag_to_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 15.0);
}

#[test]
fn zero_tick_period_updates_remove_single_stack_expiration() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        3,
        StackMagnitudePolicy::Linear,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::ResetOnSuccessfulStack,
        StackOverflowPolicy::RejectApplication,
        StackExpirationPolicy::RemoveSingleStack,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(0.0), false)),
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, armor), 20.0);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn overflow_refresh_duration_extends_existing_stack_lifetime() {
    let mut app = test_app();
    let armor = register_attribute(&mut app, "Armor");
    let target = spawn_attribute_set(&mut app, armor, 10.0);
    let stacking = StackingPolicy::new(
        StackingType::AggregateByTarget,
        1,
        StackMagnitudePolicy::None,
        StackDurationPolicy::RefreshOnSuccessfulStack,
        StackPeriodPolicy::KeepCurrentTick,
        StackOverflowPolicy::RefreshDuration,
        StackExpirationPolicy::RemoveAllStacks,
    );
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(armor, 5.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
        None,
        1.0,
        stacking,
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, target, effect.clone()));
    run_effect_duration_tick(&mut app);
    assert!(apply_effect(&mut app, target, target, effect));

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 15.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, armor), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn aggregate_by_source_keeps_separate_stacks_per_source() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first_source = app.world_mut().spawn_empty().id();
    let second_source = app.world_mut().spawn_empty().id();
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateBySource, 3),
        empty_effect_tags(),
    ));

    assert!(apply_effect(&mut app, target, first_source, effect.clone()));
    assert!(apply_effect(&mut app, target, first_source, effect.clone()));
    assert!(apply_effect(&mut app, target, second_source, effect));

    assert_eq!(active_effect_handles(&app, target).len(), 2);
    assert_eq!(current_value(&mut app, target, power), 25.0);
}

#[test]
fn active_immunity_blocks_matching_incoming_effect() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));

    assert!(apply_effect(&mut app, target, source, immunity_effect));
    assert!(!apply_effect(&mut app, target, source, incoming));
    assert_eq!(current_value(&mut app, target, health), 100.0);
}

#[test]
fn queued_immunity_is_visible_to_the_next_request() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, immunity_effect, EffectPayload::new(source, None, 1));
        queue.push_application(target, incoming, EffectPayload::new(source, None, 1));
    }

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, health), 100.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn ongoing_tag_requirements_inhibit_and_restore_active_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.Enabled");
    let granted = register_tag(&mut app, "State.Buffed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![granted],
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );

    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );

    add_tag_to_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&granted)
    );
}

#[test]
fn uninhibit_missing_attribute_storage_removes_effect_fail_closed() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.StorageEnabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(active_effect_handles(&app, target).len(), 1);
    app.world_mut().entity_mut(target).remove::<AttributeSet>();
    add_tag_to_entity(&mut app, target, enabled);

    run_effect_tag_requirements_update(&mut app);

    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn queued_granted_tag_converges_ongoing_requirements_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocked = register_tag(&mut app, "State.Blocked");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![blocked]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![blocked]),
    ));

    assert!(apply_effect(&mut app, target, target, buff));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, blocker, EffectPayload::new(target, None, 1));

    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn direct_effect_application_converges_requirements_before_returning() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocked = register_tag(&mut app, "State.DirectBlocked");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![blocked]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![blocked]),
    ));

    assert!(apply_effect(&mut app, target, target, buff));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(apply_effect(&mut app, target, target, blocker));
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn requirement_converges_between_two_gameplay_requests() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let source_tag = register_tag(&mut app, "Source.Player");
    let incoming_tag = register_tag(&mut app, "Effect.Damage.Fire");
    let disable_immunity = register_tag(&mut app, "State.DisableImmunity");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    add_tag_to_entity(&mut app, source, source_tag);

    let immunity = GameplayEffectImmunityQuery::new(
        TagRequirements::new(vec![source_tag], Vec::new()).unwrap(),
        TagRequirements::new(vec![incoming_tag], Vec::new()).unwrap(),
    );
    let immunity_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(Vec::new(), vec![disable_immunity]).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            vec![immunity],
            Vec::new(),
        ),
    ));
    let disable_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![disable_immunity]),
    ));
    let incoming = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -25.0)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![incoming_tag], Vec::new()),
    ));
    assert!(apply_effect(&mut app, target, source, immunity_effect));

    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue.push_application(target, disable_effect, EffectPayload::new(source, None, 1));
        queue.push_application(target, incoming, EffectPayload::new(source, None, 1));
    }
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, health), 75.0);
}

#[test]
fn queued_tag_source_removal_restores_effect_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let blocker_asset = register_tag(&mut app, "Effect.Blocker");
    let disabled = register_tag(&mut app, "State.Disabled");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let blocker = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![blocker_asset], vec![disabled]),
    ));
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(Vec::new(), vec![disabled]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            vec![blocker_asset],
        ),
    ));

    assert!(apply_effect(&mut app, target, target, blocker));
    assert!(apply_effect(&mut app, target, target, buff));
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);

    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, cleanse, EffectPayload::new(target, None, 1));
    run_fixed_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 15.0);
}

#[test]
fn queued_tag_triggers_removal_requirement_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let cleanse_tag = register_tag(&mut app, "State.Cleansed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let removable = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::new(vec![cleanse_tag], Vec::new()).unwrap(),
        ),
    ));
    let cleanse = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cleanse_tag]),
    ));

    assert!(apply_effect(&mut app, target, target, removable));
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, cleanse, EffectPayload::new(target, None, 1));
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert_eq!(active_effect_handles(&app, target).len(), 1);
}

#[test]
fn source_tag_changes_converge_effect_on_another_target() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "Source.Enabled");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    add_tag_to_entity(&mut app, source, enabled);
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            Vec::new(),
        ),
    ));

    assert!(apply_effect(&mut app, target, source, buff));
    assert_eq!(current_value(&mut app, target, power), 15.0);
    remove_tag_from_entity(&mut app, source, enabled);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
}

#[test]
fn queued_source_tag_restores_other_target_effect_in_same_tick() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "Source.QueueEnabled");
    let source = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let buff = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        EffectTags::new(
            Vec::new(),
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
            TagRequirements::default(),
            TagRequirements::default(),
            Vec::new(),
            Vec::new(),
        ),
    ));
    let source_enabler = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![enabled]),
    ));

    assert!(apply_effect(&mut app, target, source, buff));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    app.world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(source, source_enabler, EffectPayload::new(source, None, 1));

    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 15.0);
}

#[test]
fn duration_tag_cleanup_converges_before_period_execution() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let enabled = register_tag(&mut app, "State.Enabled");
    let attributes = attribute_set(&app, health, 100.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let enable_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![enabled]),
    ));
    let periodic_damage = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, -10.0)],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        Some(EffectPeriodTicks::new(ModifierMagnitude::Flat(1.0), false)),
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, enable_effect));
    assert!(apply_effect(&mut app, target, target, periodic_damage));
    run_fixed_update(&mut app);

    assert_eq!(current_value(&mut app, target, health), 100.0);
}

#[test]
fn non_converging_requirement_cycle_fails_closed() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let loop_tag = register_tag(&mut app, "State.RequirementLoop");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let looping_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![loop_tag],
            TagRequirements::new(Vec::new(), vec![loop_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, looping_effect));
    run_effect_tag_requirements_update(&mut app);

    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&loop_tag)
    );
}

#[test]
fn mutual_requirement_cycle_is_independent_of_effect_slot_order() {
    let mut app = test_app();
    let first_tag = register_tag(&mut app, "State.FirstLoop");
    let second_tag = register_tag(&mut app, "State.SecondLoop");
    let first_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![first_tag],
            TagRequirements::new(Vec::new(), vec![second_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let second_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![second_tag],
            TagRequirements::new(Vec::new(), vec![first_tag]).unwrap(),
            TagRequirements::default(),
        ),
    ));
    let first_target = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let second_target = app.world_mut().spawn(GameplayTagContainer::default()).id();

    assert!(apply_effect(
        &mut app,
        first_target,
        first_target,
        first_effect.clone()
    ));
    assert!(apply_effect(
        &mut app,
        first_target,
        first_target,
        second_effect.clone()
    ));
    assert!(apply_effect(
        &mut app,
        second_target,
        second_target,
        second_effect
    ));
    assert!(apply_effect(
        &mut app,
        second_target,
        second_target,
        first_effect
    ));

    run_effect_tag_requirements_update(&mut app);
    assert!(active_effect_handles(&app, first_target).is_empty());
    assert!(active_effect_handles(&app, second_target).is_empty());
}

#[test]
fn removal_tag_requirement_cleans_up_active_effect() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let cleanse = register_tag(&mut app, "State.Cleansed");
    let attributes = attribute_set(&app, power, 10.0);
    let target = app
        .world_mut()
        .spawn((GameplayTagContainer::default(), attributes))
        .id();
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            Vec::new(),
            TagRequirements::default(),
            TagRequirements::new(vec![cleanse], Vec::new()).unwrap(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 15.0);

    add_tag_to_entity(&mut app, target, cleanse);
    run_effect_tag_requirements_update(&mut app);
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn tag_granting_effect_without_tag_container_rolls_back_modifiers() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let granted = register_tag(&mut app, "State.Buffed");
    let target = spawn_attribute_set(&mut app, power, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 5.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted]),
    ));

    assert!(!apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(active_effect_handles(&app, target).is_empty());
}
