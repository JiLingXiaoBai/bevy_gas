use super::support_test::{
    active_effect_handles, add_modifier, apply_effect_result, apply_effect_with_payload_result,
    attribute_set, current_value, effect_tags, empty_effect_tags, register_attribute,
    register_hot_attribute, register_tag, run_effect_duration_tick, run_effect_period_tick,
    run_fixed_update, spawn_attribute_set, test_app,
};
use bevy::prelude::*;
use bevy_gas::{
    ActiveEffectHandle, ActiveGameplayEffects, AttributeId, AttributeIdManager, AttributeSetError,
    EffectDurationTicks, EffectPayload, EffectPeriodTicks, EffectTags, GameplayEffect,
    GameplayEffectApplicationError, GameplayExecutionError, GameplayExecutionOutcome,
    GameplayExecutionQueue, GameplayExecutionResult, GameplayTagContainer, Modifier,
    ModifierEvaluationContext, ModifierMagnitude, ModifierMagnitudeCalculation, ModifierOperation,
    ModifierSourceId, ModifierSpec, StackingPolicy, StackingType, TagRequirements,
};
use std::sync::Arc;

struct ConstantMagnitude(f32);

impl ModifierMagnitudeCalculation for ConstantMagnitude {
    fn calculate(&self, _: &dyn ModifierEvaluationContext) -> f32 {
        self.0
    }
}

struct LevelMagnitude {
    first_level: f32,
    later_levels: f32,
}

impl ModifierMagnitudeCalculation for LevelMagnitude {
    fn calculate(&self, context: &dyn ModifierEvaluationContext) -> f32 {
        if context.level() == 1 {
            self.first_level
        } else {
            self.later_levels
        }
    }
}

#[derive(Clone, Copy)]
enum EffectMode {
    Instant,
    Duration,
    Periodic { execute_on_applied: bool },
}

#[test]
fn direct_attribute_mutation_rejects_nonfinite_specs_without_replacing_contributions() {
    let mut app = test_app();
    let hot = register_hot_attribute(&mut app, "Hot");
    let cold = register_attribute(&mut app, "Cold");
    let manager = app.world().resource::<AttributeIdManager>();
    let source = ModifierSourceId::new(1, 0, 1);
    for id in [hot, cold] {
        let mut attributes = attribute_set(&app, id, 10.0);
        attributes
            .apply_duration_modifier(
                manager,
                &ModifierSpec::new(id, ModifierOperation::AddBase, 5.0),
                source,
            )
            .unwrap();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let invalid = ModifierSpec::new(id, ModifierOperation::AddBase, value);
            assert_eq!(
                attributes.apply_instant_modifier(manager, &invalid),
                Err(AttributeSetError::NonFiniteModifier { id }),
            );
            assert_eq!(attributes.get_current_value(manager, id), Ok(Some(15.0)));
            assert_eq!(
                attributes.apply_duration_modifier(manager, &invalid, source),
                Err(AttributeSetError::NonFiniteModifier { id }),
            );
            assert_eq!(attributes.get_current_value(manager, id), Ok(Some(15.0)));
        }
        attributes.remove_modifiers(source);
        assert_eq!(attributes.get_current_value(manager, id), Ok(Some(10.0)));
    }
}

#[test]
fn flat_and_calculated_nonfinite_modifiers_are_rejected_before_any_effect_mutation() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for calculated in [false, true] {
            for mode in [EffectMode::Instant, EffectMode::Duration] {
                assert_invalid_effect_is_atomic(magnitude(value, calculated), mode, false);
            }
        }
    }
}

#[test]
fn periodic_modifiers_are_validated_even_when_the_first_execution_is_deferred() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for calculated in [false, true] {
            assert_invalid_effect_is_atomic(
                magnitude(value, calculated),
                EffectMode::Periodic {
                    execute_on_applied: false,
                },
                false,
            );
        }
    }
}

#[test]
fn ongoing_inhibition_does_not_allow_invalid_modifiers_into_active_storage() {
    for (value, mode, calculated) in [
        (f32::NAN, EffectMode::Duration, true),
        (
            f32::INFINITY,
            EffectMode::Periodic {
                execute_on_applied: false,
            },
            false,
        ),
        (
            f32::NEG_INFINITY,
            EffectMode::Periodic {
                execute_on_applied: true,
            },
            true,
        ),
    ] {
        assert_invalid_effect_is_atomic(magnitude(value, calculated), mode, true);
    }
}

#[test]
fn nonfinite_modifier_requests_publish_execution_failures_instead_of_gameplay_rejections() {
    let mut app = test_app();
    let health = register_attribute(&mut app, "Health");
    let target = spawn_attribute_set(&mut app, health, 10.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 5.0), add_modifier(health, f32::NAN)],
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    let request_id = app
        .world_mut()
        .resource_mut::<GameplayExecutionQueue>()
        .push_application(target, effect, EffectPayload::new(target, None, 1))
        .unwrap();
    run_fixed_update(&mut app);
    let messages = app.world().resource::<Messages<GameplayExecutionResult>>();
    let mut cursor = messages.get_cursor();
    let results = cursor.read(messages).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_id, request_id);
    assert_eq!(
        results[0].outcome,
        GameplayExecutionOutcome::Failed(GameplayExecutionError::EffectApplication(
            GameplayEffectApplicationError::NonFiniteModifier { target, id: health },
        )),
    );
    assert_eq!(current_value(&mut app, target, health), 10.0);
}

#[test]
fn linear_stack_overflow_uses_retained_specs_and_preserves_existing_runtime_state() {
    let large = f32::MAX * 0.75;
    assert!(large.is_finite());
    assert!(!(large * 2.0).is_finite());
    for periodic in [false, true] {
        let mut app = test_app();
        let power = register_attribute(&mut app, "Power");
        let target = spawn_attribute_set(&mut app, power, 0.0);
        let effect = stacking_effect(power, large, 1.0, periodic);
        apply_effect_with_payload_result(
            &mut app,
            target,
            Arc::clone(&effect),
            EffectPayload::new(target, None, 1),
        )
        .unwrap();
        let handle = active_effect_handles(&app, target)[0];
        run_effect_duration_tick(&mut app);
        if periodic {
            run_effect_period_tick(&mut app);
        }
        let before = stack_state(&app, target, handle);
        let contribution = current_value(&mut app, target, power);
        assert_eq!(before, (1, 9, periodic.then_some(1), large));
        assert_eq!(contribution, if periodic { 0.0 } else { large });

        // The new evaluation is small; the retained level-one magnitude overflows at two stacks.
        assert_eq!(
            apply_effect_with_payload_result(
                &mut app,
                target,
                effect,
                EffectPayload::new(target, None, 2)
            ),
            Err(GameplayEffectApplicationError::NonFiniteModifier { target, id: power }),
        );
        assert_eq!(active_effect_handles(&app, target), vec![handle]);
        assert_eq!(stack_state(&app, target, handle), before);
        assert_eq!(current_value(&mut app, target, power), contribution);
    }
}

#[test]
fn valid_linear_stacks_do_not_scale_the_unused_incoming_evaluation() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 0.0);
    let effect = stacking_effect(power, 1.0, f32::MAX * 0.75, false);
    apply_effect_with_payload_result(
        &mut app,
        target,
        Arc::clone(&effect),
        EffectPayload::new(target, None, 1),
    )
    .unwrap();
    let handle = active_effect_handles(&app, target)[0];
    run_effect_duration_tick(&mut app);

    // Incoming magnitudes are finite, but only the retained first-level specification is scaled.
    apply_effect_with_payload_result(
        &mut app,
        target,
        effect,
        EffectPayload::new(target, None, 2),
    )
    .unwrap();
    assert_eq!(active_effect_handles(&app, target), vec![handle]);
    assert_eq!(stack_state(&app, target, handle), (2, 10, None, 1.0));
    assert_eq!(current_value(&mut app, target, power), 2.0);
}

fn magnitude(value: f32, calculated: bool) -> ModifierMagnitude {
    if calculated {
        ModifierMagnitude::Calculated(Box::new(ConstantMagnitude(value)))
    } else {
        ModifierMagnitude::Flat(value)
    }
}

fn assert_invalid_effect_is_atomic(invalid: ModifierMagnitude, mode: EffectMode, inhibited: bool) {
    let mut app = test_app();
    let health = register_hot_attribute(&mut app, "Health");
    let power = register_attribute(&mut app, "Power");
    let removable = register_tag(&mut app, "Effect.Removable");
    let existing_tag = register_tag(&mut app, "State.Existing");
    let incoming_tag = register_tag(&mut app, "State.Incoming");
    let enabled = register_tag(&mut app, "State.Enabled");
    let mut attributes = attribute_set(&app, health, 10.0);
    attributes
        .initialize_attribute(
            app.world().resource::<AttributeIdManager>(),
            power,
            20.0,
            None,
        )
        .unwrap();
    let target = app
        .world_mut()
        .spawn((
            attributes,
            ActiveGameplayEffects::default(),
            GameplayTagContainer::default(),
        ))
        .id();
    let existing = Arc::new(GameplayEffect::new(
        vec![add_modifier(health, 3.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![removable], vec![existing_tag]),
    ));
    apply_effect_result(&mut app, target, target, existing).unwrap();
    let old_handles = active_effect_handles(&app, target);
    let mut tags = EffectTags::new(Vec::new(), vec![incoming_tag])
        .with_remove_effects_with_tags(vec![removable]);
    if inhibited {
        tags = tags.with_ongoing_requirements(
            TagRequirements::default(),
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
        );
    }
    let (duration, period) = match mode {
        EffectMode::Instant => (EffectDurationTicks::Instant, None),
        EffectMode::Duration => (
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
            None,
        ),
        EffectMode::Periodic { execute_on_applied } => (
            EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
            Some(EffectPeriodTicks::new(
                ModifierMagnitude::Flat(3.0),
                execute_on_applied,
            )),
        ),
    };
    let incoming = Arc::new(GameplayEffect::new(
        vec![
            add_modifier(health, 5.0),
            Modifier::new(power, ModifierOperation::AddBase, invalid),
        ],
        duration,
        period,
        1.0,
        StackingPolicy::non_stacking(),
        tags,
    ));
    let error = apply_effect_result(&mut app, target, target, incoming).unwrap_err();
    assert_eq!(
        error,
        GameplayEffectApplicationError::NonFiniteModifier { target, id: power }
    );
    assert!(!error.is_rejection());
    assert_eq!(active_effect_handles(&app, target), old_handles);
    assert_eq!(current_value(&mut app, target, health), 13.0);
    assert_eq!(current_value(&mut app, target, power), 20.0);
    let tags = app.world().get::<GameplayTagContainer>(target).unwrap();
    assert!(tags.has_tag(&existing_tag));
    assert!(!tags.has_tag(&incoming_tag));
}

fn stacking_effect(
    attribute: AttributeId,
    first_level: f32,
    later_levels: f32,
    periodic: bool,
) -> Arc<GameplayEffect> {
    Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            attribute,
            ModifierOperation::AddBase,
            ModifierMagnitude::Calculated(Box::new(LevelMagnitude {
                first_level,
                later_levels,
            })),
        )],
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(10.0)),
        periodic.then(|| EffectPeriodTicks::new(ModifierMagnitude::Flat(3.0), false)),
        1.0,
        StackingPolicy::linear_refreshing(StackingType::AggregateByTarget, 3),
        empty_effect_tags(),
    ))
}

fn stack_state(
    app: &App,
    target: Entity,
    handle: ActiveEffectHandle,
) -> (u32, u32, Option<u32>, f32) {
    let effect = app
        .world()
        .get::<ActiveGameplayEffects>(target)
        .unwrap()
        .get(handle)
        .unwrap();
    (
        effect.get_stack_count(),
        effect.get_duration().unwrap().get_remaining_ticks(),
        effect.get_period().map(|period| period.get_current_tick()),
        effect.get_spec().get_modifier_specs()[0].get_value(),
    )
}
