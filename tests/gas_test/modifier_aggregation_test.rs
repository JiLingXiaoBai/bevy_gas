use super::support_test::{
    active_effect_handles, apply_effect, current_value, empty_effect_tags, modifier,
    register_attribute, run_effect_duration_tick, spawn_attribute_set, test_app,
};
use bevy_gas::{
    ActiveGameplayEffects, Aggregator, AttributeId, AttributeIdManager, AttributeSet,
    EffectDurationTicks, GameplayEffect, ModifierMagnitude, ModifierOperation, ModifierSourceId,
    ModifierSpec, StackDurationPolicy, StackExpirationPolicy, StackMagnitudePolicy,
    StackOverflowPolicy, StackPeriodPolicy, StackingPolicy, StackingType,
};
use std::sync::Arc;

#[test]
fn operation_discriminants_match_ue() {
    for (operation, expected) in [
        (ModifierOperation::AddBase, 0),
        (ModifierOperation::MultiplyAdditive, 1),
        (ModifierOperation::DivideAdditive, 2),
        (ModifierOperation::Override, 3),
        (ModifierOperation::MultiplyCompound, 4),
        (ModifierOperation::AddFinal, 5),
    ] {
        assert_eq!(operation as u8, expected);
    }
}

fn effect(
    attribute: AttributeId,
    operations: &[(ModifierOperation, f32)],
    duration: EffectDurationTicks,
) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        operations
            .iter()
            .map(|&(operation, value)| modifier(attribute, operation, value))
            .collect(),
        duration,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ))
}

#[test]
fn duration_pipeline_combines_biased_divisors_before_compound_multipliers_and_final_adds() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    let buff = effect(
        power,
        &[
            (ModifierOperation::AddFinal, 5.0),
            (ModifierOperation::MultiplyCompound, 2.0),
            (ModifierOperation::DivideAdditive, 2.0),
            (ModifierOperation::MultiplyAdditive, 1.5),
            (ModifierOperation::AddBase, 20.0),
            (ModifierOperation::DivideAdditive, 3.0),
        ],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
    );

    assert!(apply_effect(&mut app, target, target, buff));
    // (100 + 20) * 1.5 / (1 + (2 - 1) + (3 - 1)) * 2 + 5.
    assert_eq!(current_value(&mut app, target, power), 95.0);
    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, power), 95.0);
    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, power), 100.0);
    assert!(active_effect_handles(&app, target).is_empty());
}

#[test]
fn additive_and_compound_multipliers_group_duration_contributions_differently() {
    for (operation, duration, expected) in [
        (
            ModifierOperation::MultiplyAdditive,
            EffectDurationTicks::Infinite,
            200.0,
        ),
        (
            ModifierOperation::MultiplyCompound,
            EffectDurationTicks::Infinite,
            225.0,
        ),
        (
            ModifierOperation::MultiplyAdditive,
            EffectDurationTicks::Instant,
            225.0,
        ),
        (
            ModifierOperation::MultiplyCompound,
            EffectDurationTicks::Instant,
            225.0,
        ),
    ] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let target = spawn_attribute_set(&mut app, power, 100.0);
        let adjustment = effect(power, &[(operation, 1.5), (operation, 1.5)], duration);

        assert!(apply_effect(&mut app, target, target, adjustment));
        assert_eq!(current_value(&mut app, target, power), expected);
    }
}

#[test]
fn stack_scaling_uses_operation_bias_and_preserves_override() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    for (operation, value, expected) in [
        (ModifierOperation::AddBase, 7.0, [0.0, 7.0, 21.0]),
        (ModifierOperation::MultiplyAdditive, 1.5, [1.0, 1.5, 2.5]),
        (ModifierOperation::DivideAdditive, 1.5, [1.0, 1.5, 2.5]),
        (ModifierOperation::Override, 7.0, [7.0, 7.0, 7.0]),
        (ModifierOperation::MultiplyCompound, 1.5, [0.0, 1.5, 4.5]),
        (ModifierOperation::AddFinal, 7.0, [0.0, 7.0, 21.0]),
    ] {
        let spec = ModifierSpec::new(power, operation, value);
        for (count, expected) in [0, 1, 3].into_iter().zip(expected) {
            let scaled = spec.scaled_by_stack(count);
            assert_eq!(scaled.get_id(), power);
            assert_eq!(scaled.get_operation(), operation);
            assert_eq!(scaled.get_value(), expected);
        }
    }
}

#[test]
fn stacked_magnitudes_normalize_negative_zero_except_override() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    for operation in [
        ModifierOperation::AddBase,
        ModifierOperation::MultiplyCompound,
        ModifierOperation::AddFinal,
    ] {
        let value = ModifierSpec::new(power, operation, -0.0)
            .scaled_by_stack(1)
            .get_value();
        assert_eq!(value, 0.0);
        assert!(value.is_sign_positive());
    }
    let overridden = ModifierSpec::new(power, ModifierOperation::Override, -0.0)
        .scaled_by_stack(1)
        .get_value();
    assert_eq!(overridden, 0.0);
    assert!(overridden.is_sign_negative());
}

#[test]
fn each_stacked_operation_recalculates_after_single_stack_expiration() {
    for (operation, magnitude, one_stack, two_stacks) in [
        (ModifierOperation::AddBase, 5.0, 105.0, 110.0),
        (ModifierOperation::MultiplyAdditive, 1.5, 150.0, 200.0),
        (ModifierOperation::DivideAdditive, 1.5, 100.0 / 1.5, 50.0),
        (ModifierOperation::Override, 42.0, 42.0, 42.0),
        (ModifierOperation::MultiplyCompound, 1.5, 150.0, 300.0),
        (ModifierOperation::AddFinal, 5.0, 105.0, 110.0),
    ] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let target = spawn_attribute_set(&mut app, power, 100.0);
        let stacking = StackingPolicy::new(
            StackingType::AggregateByTarget,
            2,
            StackMagnitudePolicy::Linear,
            StackDurationPolicy::RefreshOnSuccessfulStack,
            StackPeriodPolicy::KeepCurrentTick,
            StackOverflowPolicy::RejectApplication,
            StackExpirationPolicy::RemoveSingleStack,
        );
        let adjustment = Arc::new(GameplayEffect::new(
            vec![modifier(power, operation, magnitude)],
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
            None,
            1.0,
            stacking,
            empty_effect_tags(),
        ));

        assert!(apply_effect(&mut app, target, target, adjustment.clone()));
        assert_eq!(current_value(&mut app, target, power), one_stack);
        assert!(apply_effect(&mut app, target, target, adjustment));
        assert_eq!(current_value(&mut app, target, power), two_stacks);
        assert_eq!(active_effect_handles(&app, target).len(), 1);
        run_effect_duration_tick(&mut app);
        assert_eq!(current_value(&mut app, target, power), one_stack);
        run_effect_duration_tick(&mut app);
        assert_eq!(current_value(&mut app, target, power), 100.0);
        assert!(active_effect_handles(&app, target).is_empty());
    }
}

#[test]
fn instant_final_add_and_division_execute_in_definition_order() {
    for (operations, expected) in [
        (
            [
                (ModifierOperation::AddFinal, 10.0),
                (ModifierOperation::MultiplyCompound, 2.0),
                (ModifierOperation::DivideAdditive, 2.0),
            ],
            110.0,
        ),
        (
            [
                (ModifierOperation::DivideAdditive, 2.0),
                (ModifierOperation::AddFinal, 10.0),
                (ModifierOperation::MultiplyCompound, 2.0),
            ],
            120.0,
        ),
    ] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let target = spawn_attribute_set(&mut app, power, 100.0);
        let instant = effect(power, &operations, EffectDurationTicks::Instant);

        assert!(apply_effect(&mut app, target, target, instant));
        assert_eq!(current_value(&mut app, target, power), expected);
        let manager = app.world().resource::<AttributeIdManager>();
        let snapshot = app
            .world()
            .get::<AttributeSet>(target)
            .unwrap()
            .evaluate_attribute(manager, power)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.base(), expected);
        assert!(active_effect_handles(&app, target).is_empty());
    }
}

#[test]
fn readonly_modifiers_expose_all_operation_values_sources_and_current_order() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first = ModifierSourceId::new(10, 1, 1);
    let second = ModifierSourceId::new(10, 2, 1);
    let operations = [
        ModifierOperation::AddBase,
        ModifierOperation::MultiplyAdditive,
        ModifierOperation::MultiplyCompound,
        ModifierOperation::DivideAdditive,
        ModifierOperation::AddFinal,
        ModifierOperation::Override,
    ];
    let mut aggregator = Aggregator::default();
    for (index, operation) in operations.into_iter().enumerate() {
        aggregator.apply_modifier_spec(
            &ModifierSpec::new(power, operation, index as f32 + 1.0),
            first,
        );
        aggregator.apply_modifier_spec(
            &ModifierSpec::new(power, operation, index as f32 + 11.0),
            second,
        );
    }

    for (index, operation) in operations.into_iter().enumerate() {
        let retained: Vec<_> = aggregator
            .modifiers(operation)
            .map(|modifier| (modifier.get_source_id(), modifier.get_value()))
            .collect();
        let expected = vec![(first, index as f32 + 1.0), (second, index as f32 + 11.0)];
        assert_eq!(retained, expected);
    }
    assert_eq!(aggregator.modifier_count(), 12);
    assert_eq!(aggregator.evaluate(100.0), 6.0);

    aggregator.remove_modifiers_by_source(first);
    assert_eq!(aggregator.modifier_count(), 6);
    assert_eq!(aggregator.evaluate(100.0), 16.0);

    aggregator.reset();
    for operation in operations {
        assert!(aggregator.modifiers(operation).next().is_none());
    }
    assert_eq!(aggregator.modifier_count(), 0);
    assert_eq!(aggregator.evaluate(100.0), 100.0);
}

#[test]
fn source_removal_uses_swap_order_in_every_operation_bucket() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first = ModifierSourceId::new(13, 1, 1);
    let second = ModifierSourceId::new(13, 2, 1);
    let third = ModifierSourceId::new(13, 3, 1);
    for operation in [
        ModifierOperation::AddBase,
        ModifierOperation::MultiplyAdditive,
        ModifierOperation::DivideAdditive,
        ModifierOperation::Override,
        ModifierOperation::MultiplyCompound,
        ModifierOperation::AddFinal,
    ] {
        let mut aggregator = Aggregator::default();
        for (source, value) in [(first, 10.0), (second, 20.0), (first, 11.0), (third, 30.0)] {
            aggregator.apply_modifier_spec(&ModifierSpec::new(power, operation, value), source);
        }
        aggregator.remove_modifiers_by_source(first);
        let retained: Vec<_> = aggregator
            .modifiers(operation)
            .map(|m| m.get_value())
            .collect();
        assert_eq!(retained, [30.0, 20.0]);
        assert_eq!(aggregator.modifier_count(), 2);
        if operation == ModifierOperation::Override {
            assert_eq!(aggregator.evaluate(100.0), 30.0);
        }
    }
}

#[test]
fn expiring_override_selects_first_remaining_candidate_after_swap_removal() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    for (value, duration) in [
        (
            10.0,
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        ),
        (
            20.0,
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(3.0)),
        ),
        (
            30.0,
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
        ),
    ] {
        let adjustment = effect(power, &[(ModifierOperation::Override, value)], duration);
        assert!(apply_effect(&mut app, target, target, adjustment));
    }
    assert_eq!(current_value(&mut app, target, power), 10.0);
    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, power), 30.0);
    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, power), 100.0);
}

#[test]
fn refreshing_an_override_stack_reinserts_its_contribution_after_other_candidates() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    let stacked = Arc::new(GameplayEffect::new(
        vec![modifier(power, ModifierOperation::Override, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 2),
        empty_effect_tags(),
    ));
    let other = effect(
        power,
        &[(ModifierOperation::Override, 20.0)],
        EffectDurationTicks::Infinite,
    );

    assert!(apply_effect(&mut app, target, target, stacked.clone()));
    assert!(apply_effect(&mut app, target, target, other));
    assert_eq!(current_value(&mut app, target, power), 10.0);
    assert!(apply_effect(&mut app, target, target, stacked));
    assert_eq!(current_value(&mut app, target, power), 20.0);
    assert_eq!(active_effect_handles(&app, target).len(), 2);
}

#[test]
fn source_removal_clears_divisors_and_final_adds_together() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first = ModifierSourceId::new(11, 1, 1);
    let second = ModifierSourceId::new(11, 2, 1);
    let mut aggregator = Aggregator::default();
    for (source, divisor, final_add) in [(first, 2.0, 5.0), (second, 3.0, 20.0)] {
        aggregator.apply_modifier_spec(
            &ModifierSpec::new(power, ModifierOperation::DivideAdditive, divisor),
            source,
        );
        aggregator.apply_modifier_spec(
            &ModifierSpec::new(power, ModifierOperation::AddFinal, final_add),
            source,
        );
    }
    assert_eq!(aggregator.evaluate(120.0), 55.0);

    aggregator.remove_modifiers_by_source(first);
    assert_eq!(aggregator.modifier_count(), 2);
    assert_eq!(aggregator.evaluate(120.0), 60.0);
    aggregator.remove_modifiers_by_source(second);
    assert_eq!(aggregator.modifier_count(), 0);
    assert_eq!(aggregator.evaluate(120.0), 120.0);
}

#[test]
fn zero_aggregate_divisor_uses_identity_including_after_source_removal() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let first = ModifierSourceId::new(12, 1, 1);
    let second = ModifierSourceId::new(12, 2, 1);
    let third = ModifierSourceId::new(12, 3, 1);
    let mut aggregator = Aggregator::default();
    for source in [first, second] {
        aggregator.apply_modifier_spec(
            &ModifierSpec::new(power, ModifierOperation::DivideAdditive, 0.5),
            source,
        );
    }
    assert_eq!(aggregator.evaluate(100.0), 100.0);

    aggregator.apply_modifier_spec(
        &ModifierSpec::new(power, ModifierOperation::DivideAdditive, 3.0),
        third,
    );
    assert_eq!(aggregator.evaluate(100.0), 50.0);
    aggregator.remove_modifiers_by_source(third);
    assert_eq!(aggregator.evaluate(100.0), 100.0);
    aggregator.remove_modifiers_by_source(first);
    assert_eq!(aggregator.evaluate(100.0), 200.0);
}

#[test]
fn near_zero_divisors_are_identity_and_larger_negative_divisors_remain_signed() {
    for (divisor, expected) in [
        (0.0, 100.0),
        (-0.0, 100.0),
        (1.0e-8, 100.0),
        (-1.0e-8, 100.0),
        (2.0_f32.powi(-20), 104_857_600.0),
        (-2.0_f32.powi(-20), -104_857_600.0),
        (-2.0, -50.0),
    ] {
        for duration in [EffectDurationTicks::Instant, EffectDurationTicks::Infinite] {
            let mut app = test_app();
            let power = register_attribute(&mut app, "Power");
            let target = spawn_attribute_set(&mut app, power, 100.0);
            let adjustment = effect(
                power,
                &[(ModifierOperation::DivideAdditive, divisor)],
                duration,
            );

            assert!(apply_effect(&mut app, target, target, adjustment));
            assert_eq!(current_value(&mut app, target, power), expected);
        }
    }
}

#[test]
fn direct_instant_division_applies_the_near_zero_tolerance_to_the_raw_magnitude() {
    let above_tolerance = 1.000_001e-8_f32;
    for (divisor, expected) in [
        (1.0e-8, 100.0),
        (-1.0e-8, 100.0),
        (above_tolerance, 100.0 / above_tolerance),
        (-above_tolerance, -100.0 / above_tolerance),
    ] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let manager = app.world().resource::<AttributeIdManager>();
        let mut attributes = AttributeSet::default();
        attributes
            .initialize_attribute(manager, power, 100.0, None)
            .unwrap();
        attributes
            .apply_instant_modifier(
                manager,
                &ModifierSpec::new(power, ModifierOperation::DivideAdditive, divisor),
            )
            .unwrap();
        assert_eq!(
            attributes.get_current_value(manager, power),
            Ok(Some(expected))
        );
    }
}

#[test]
fn instant_effect_uses_single_stack_bias_rounding_before_division() {
    for divisor in [1.000_001e-8, -1.000_001e-8] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let spec = ModifierSpec::new(power, ModifierOperation::DivideAdditive, divisor);
        assert_eq!(spec.scaled_by_stack(1).get_value(), 0.0);
        let target = spawn_attribute_set(&mut app, power, 100.0);
        let adjustment = effect(
            power,
            &[(ModifierOperation::DivideAdditive, divisor)],
            EffectDurationTicks::Instant,
        );
        assert!(apply_effect(&mut app, target, target, adjustment));
        assert_eq!(current_value(&mut app, target, power), 100.0);
    }
}

fn strongest_percentage_slow(aggregator: &Aggregator, base: f32) -> f32 {
    let mut bonus = 0.0;
    let mut slow = 0.0_f32;
    for modifier in aggregator.modifiers(ModifierOperation::MultiplyAdditive) {
        let value = modifier.get_value() - 1.0;
        if value >= 0.0 {
            bonus += value;
        } else {
            slow = slow.min(value);
        }
    }
    base * (1.0 + bonus + slow)
}

#[test]
fn custom_readonly_aggregation_restores_weaker_slow_when_stronger_effect_expires() {
    let mut app = test_app();
    let speed = register_attribute(&mut app, "Speed");
    let manager = app.world().resource::<AttributeIdManager>();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(manager, speed, 600.0, Some(strongest_percentage_slow))
        .unwrap();
    let target = app
        .world_mut()
        .spawn((attributes, ActiveGameplayEffects::default()))
        .id();
    for (value, duration) in [
        (1.25, EffectDurationTicks::Infinite),
        (0.75, EffectDurationTicks::Infinite),
        (
            0.5,
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(1.0)),
        ),
    ] {
        let modifier = effect(
            speed,
            &[(ModifierOperation::MultiplyAdditive, value)],
            duration,
        );
        assert!(apply_effect(&mut app, target, target, modifier));
    }

    let manager = app.world().resource::<AttributeIdManager>();
    let snapshot = app
        .world()
        .get::<AttributeSet>(target)
        .unwrap()
        .evaluate_attribute(manager, speed)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.base(), 600.0);
    assert_eq!(snapshot.current(), 450.0);
    assert_eq!(current_value(&mut app, target, speed), 450.0);
    assert_eq!(active_effect_handles(&app, target).len(), 3);

    run_effect_duration_tick(&mut app);
    assert_eq!(current_value(&mut app, target, speed), 600.0);
    assert_eq!(active_effect_handles(&app, target).len(), 2);
}
