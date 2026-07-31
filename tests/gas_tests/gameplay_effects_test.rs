use super::common_test::{
    active_effect_handles, add_modifier, add_tag_to_entity, apply_effect, attribute_set,
    current_value, effect_tags, empty_effect_tags, register_attribute, register_tag,
    run_effect_duration_tick, run_effect_period_tick, run_effect_tag_requirements_update,
    spawn_attribute_set, test_app,
};
use bevy_tools::{
    ActiveGameplayEffect, EffectDurationTicks, EffectPeriodTicks, EffectTags, GameplayEffect,
    GameplayEffectImmunityQuery, GameplayTag, GameplayTagContainer, ModifierMagnitude,
    StackDurationPolicy, StackExpirationPolicy, StackMagnitudePolicy, StackOverflowPolicy,
    StackPeriodPolicy, StackingPolicy, StackingType, TagRequirements,
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
        .entity(handles[0])
        .get::<ActiveGameplayEffect>()
        .unwrap();
    assert_eq!(active_effect.get_stack_count(), 2);
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
            .entity(handle)
            .get::<ActiveGameplayEffect>()
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
        TagRequirements::new(vec![source_tag], Vec::new()),
        TagRequirements::new(vec![incoming_tag], Vec::new()),
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
            TagRequirements::new(vec![enabled], Vec::new()),
            TagRequirements::default(),
        ),
    ));

    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert!(
        app.world()
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
            TagRequirements::new(vec![cleanse], Vec::new()),
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
