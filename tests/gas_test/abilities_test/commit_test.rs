use super::*;
use crate::support_test::{active_effect_handles, apply_effect, modifier, register_hot_attribute};
use bevy::ecs::system::RunSystemOnce;
use bevy_gas::{
    AbilityCommitError, AbilitySystemParams, Aggregator, AttributeIdManager, AttributeSet,
    EffectTags, can_activate_ability, commit_ability, default_executor,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn ability_activation_commits_cost_and_cooldown_then_cooldown_blocks_reactivation() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let cooldown_tag = register_tag(&mut app, "Cooldown.Fireball");
    let attributes = attribute_set(&app, mana, 50.0);
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let cost = instant_add_effect(mana, -20.0);
    let cooldown = Arc::new(bevy_gas::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        Some(cooldown),
        Some(cost),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 30.0);
    assert!(
        app.world()
            .entity(source)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&cooldown_tag)
    );

    run_finished_ability_cleanup(&mut app);
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
    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 30.0);
}

#[test]
fn ability_cost_fails_when_it_would_drop_attribute_below_zero() {
    let mut app = test_app();
    let stamina = register_attribute(&mut app, "Stamina");
    let attributes = attribute_set(&app, stamina, 10.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        Some(instant_add_effect(stamina, -20.0)),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, stamina), 10.0);
}

#[test]
fn cooldown_prepare_failure_does_not_spend_ability_cost() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let cooldown_tag = register_tag(&mut app, "Cooldown.NoContainer");
    let attributes = attribute_set(&app, mana, 50.0);
    let source = app
        .world_mut()
        .spawn((AbilitySystemComponent::default(), attributes))
        .id();
    let cooldown = Arc::new(bevy_gas::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        Some(cooldown),
        Some(instant_add_effect(mana, -20.0)),
        Vec::new(),
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(current_value(&mut app, source, mana), 50.0);
    assert_eq!(active_ability_count(&mut app), 0);
}

#[test]
fn activation_effect_failure_does_not_block_ability_success() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "Effect.MissingTargetContainer");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let best_effort_effect = Arc::new(bevy_gas::GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![tag]),
    ));
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        vec![best_effort_effect],
        true,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        1
    );
}

fn ability_with_cost_effect(cost: Arc<GameplayEffect>) -> Arc<GameplayAbility> {
    Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        Some(cost),
        Vec::new(),
        true,
        false,
    ))
}

fn ability_with_cost_modifiers(modifiers: Vec<Modifier>) -> Arc<GameplayAbility> {
    ability_with_cost_effect(Arc::new(GameplayEffect::new(
        modifiers,
        EffectDurationTicks::Instant,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    )))
}

fn can_activate_cost_ability(
    app: &mut App,
    source: Entity,
    ability: &Arc<GameplayAbility>,
) -> bool {
    let ability = Arc::clone(ability);
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            can_activate_ability(source, source, &ability, 1, &mut params)
        })
        .unwrap()
}

fn commit_cost_ability(
    app: &mut App,
    source: Entity,
    ability: &Arc<GameplayAbility>,
) -> Result<(), AbilityCommitError> {
    let ability = Arc::clone(ability);
    app.world_mut()
        .run_system_once(move |mut params: AbilitySystemParams| {
            commit_ability(source, &ability, 1, &mut params)
        })
        .unwrap()
}

fn cost_attribute_values(app: &mut App, source: Entity, attribute: AttributeId) -> (f32, f32) {
    let manager = app.world().resource::<AttributeIdManager>().clone();
    let snapshot = app
        .world_mut()
        .entity_mut(source)
        .get_mut::<AttributeSet>()
        .unwrap()
        .make_snapshot(source);
    (
        snapshot
            .get_base_value(&manager, attribute)
            .unwrap()
            .unwrap(),
        snapshot
            .get_current_value(&manager, attribute)
            .unwrap()
            .unwrap(),
    )
}

fn assert_cost_rejected(app: &mut App, source: Entity, ability: &Arc<GameplayAbility>) {
    assert!(!can_activate_cost_ability(app, source, ability));
    assert_eq!(
        commit_cost_ability(app, source, ability),
        Err(AbilityCommitError::InsufficientCost)
    );
    let handle = give_ability(app, source, Arc::clone(ability));
    assert_eq!(
        activate_ability_result(app, source, source, handle),
        Err(AbilityActivationError::CommitPreparationFailed {
            source,
            handle,
            error: AbilityCommitError::InsufficientCost,
        })
    );
    assert_eq!(active_ability_count(app), 0);
}

#[test]
fn repeated_cost_modifiers_are_checked_together_before_any_payment() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let attributes = attribute_set(&app, mana, 100.0);
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let ability = ability_with_cost_modifiers(vec![
        modifier(mana, ModifierOperation::Add, -60.0),
        modifier(mana, ModifierOperation::Add, -60.0),
    ]);

    assert_cost_rejected(&mut app, source, &ability);
    assert_eq!(
        cost_attribute_values(&mut app, source, mana),
        (100.0, 100.0)
    );
}

#[test]
fn activation_pays_all_repeated_cost_modifiers_exactly_once() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let attributes = attribute_set(&app, mana, 100.0);
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let ability = ability_with_cost_modifiers(vec![
        modifier(mana, ModifierOperation::Add, -40.0),
        modifier(mana, ModifierOperation::Add, -60.0),
    ]);

    assert!(can_activate_cost_ability(&mut app, source, &ability));
    assert_eq!(
        cost_attribute_values(&mut app, source, mana),
        (100.0, 100.0)
    );
    let handle = give_ability(&mut app, source, ability);
    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(cost_attribute_values(&mut app, source, mana), (0.0, 0.0));
}

#[test]
fn cost_affordability_uses_the_active_aggregator_after_changing_base() {
    let cases = [
        (10.0, ModifierOperation::Multiply, 2.0, -15.0, None),
        (
            20.0,
            ModifierOperation::Multiply,
            0.5,
            -15.0,
            Some((5.0, 2.5)),
        ),
        (
            10.0,
            ModifierOperation::Add,
            100.0,
            -50.0,
            Some((-40.0, 60.0)),
        ),
    ];
    for (base, operation, magnitude, cost, expected) in cases {
        let mut app = test_app();
        let mana = register_attribute(&mut app, "Mana");
        let attributes = attribute_set(&app, mana, base);
        let source = app
            .world_mut()
            .spawn(GameplayAbilitySystemBundle {
                attributes,
                ..Default::default()
            })
            .id();
        let persistent_effect = Arc::new(GameplayEffect::new(
            vec![modifier(mana, operation, magnitude)],
            EffectDurationTicks::Infinite,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            empty_effect_tags(),
        ));
        assert!(apply_effect(&mut app, source, source, persistent_effect));
        let ability = ability_with_cost_effect(instant_add_effect(mana, cost));

        // Leave current dirty until the affordability check evaluates the retained modifiers.
        if let Some(expected) = expected {
            assert!(can_activate_cost_ability(&mut app, source, &ability));
            assert_eq!(commit_cost_ability(&mut app, source, &ability), Ok(()));
            assert_eq!(cost_attribute_values(&mut app, source, mana), expected);
        } else {
            assert_cost_rejected(&mut app, source, &ability);
            assert_eq!(cost_attribute_values(&mut app, source, mana), (10.0, 20.0));
        }
        assert_eq!(active_effect_handles(&app, source).len(), 1);
    }
}

#[test]
fn interleaved_hot_and_cold_costs_accumulate_independently() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let stamina = register_hot_attribute(&mut app, "Stamina");
    let energy = register_attribute(&mut app, "Energy");
    let manager = app.world().resource::<AttributeIdManager>();
    let mut attributes = AttributeSet::default();
    for (attribute, base) in [(mana, 100.0), (stamina, 50.0), (energy, 30.0)] {
        attributes
            .initialize_attribute(manager, attribute, base, None)
            .unwrap();
    }
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let ability = ability_with_cost_modifiers(vec![
        modifier(energy, ModifierOperation::Add, -10.0),
        modifier(mana, ModifierOperation::Add, -30.0),
        modifier(stamina, ModifierOperation::Add, -20.0),
        modifier(mana, ModifierOperation::Add, -70.0),
        modifier(energy, ModifierOperation::Add, -20.0),
    ]);

    assert!(can_activate_cost_ability(&mut app, source, &ability));
    assert_eq!(commit_cost_ability(&mut app, source, &ability), Ok(()));
    assert_eq!(cost_attribute_values(&mut app, source, mana), (0.0, 0.0));
    assert_eq!(
        cost_attribute_values(&mut app, source, stamina),
        (30.0, 30.0)
    );
    assert_eq!(cost_attribute_values(&mut app, source, energy), (0.0, 0.0));
}

#[test]
fn cost_checks_preserve_modifier_order_and_reject_non_finite_values() {
    let cases = [
        (10.0, vec![-15.0, 10.0], None),
        (10.0, vec![10.0, -15.0], Some(5.0)),
        (16_777_216.0, vec![-16_777_216.0, -1.0], None),
        (10.0, vec![f32::NAN], None),
        (10.0, vec![f32::INFINITY], None),
        (10.0, vec![f32::NEG_INFINITY], None),
        (f32::MAX, vec![f32::MAX], None),
        (f32::INFINITY, vec![-1.0], None),
        (f32::NAN, vec![-1.0], None),
    ];
    for (base, costs, expected) in cases {
        let mut app = test_app();
        let mana = register_attribute(&mut app, "Mana");
        let attributes = attribute_set(&app, mana, base);
        let source = app
            .world_mut()
            .spawn(GameplayAbilitySystemBundle {
                attributes,
                ..Default::default()
            })
            .id();
        let ability = ability_with_cost_modifiers(
            costs
                .into_iter()
                .map(|cost| modifier(mana, ModifierOperation::Add, cost))
                .collect(),
        );

        if let Some(expected) = expected {
            assert!(can_activate_cost_ability(&mut app, source, &ability));
            assert_eq!(commit_cost_ability(&mut app, source, &ability), Ok(()));
            assert_eq!(
                cost_attribute_values(&mut app, source, mana),
                (expected, expected)
            );
        } else {
            assert_cost_rejected(&mut app, source, &ability);
            let (actual_base, actual_current) = cost_attribute_values(&mut app, source, mana);
            assert_eq!(actual_base.to_bits(), base.to_bits());
            assert_eq!(actual_current.to_bits(), base.to_bits());
        }
    }
}

fn doubled_cost_executor(aggregator: &Aggregator, base: f32) -> f32 {
    default_executor(aggregator, base) * 2.0
}

#[test]
fn cost_affordability_uses_custom_aggregator_executors() {
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let manager = app.world().resource::<AttributeIdManager>();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(manager, mana, 10.0, Some(doubled_cost_executor))
        .unwrap();
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let ability = ability_with_cost_effect(instant_add_effect(mana, -15.0));

    assert_cost_rejected(&mut app, source, &ability);
    assert_eq!(cost_attribute_values(&mut app, source, mana), (10.0, 20.0));
}

static COST_POST_EXECUTE_COUNT: AtomicUsize = AtomicUsize::new(0);

fn count_cost_post_execute(
    _attributes: &mut AttributeSet,
    _manager: &AttributeIdManager,
    _id: AttributeId,
    old_value: f32,
    new_value: f32,
) {
    let index = COST_POST_EXECUTE_COUNT.fetch_add(1, Ordering::SeqCst);
    let expected = [(100.0, 80.0), (80.0, 50.0)];
    assert_eq!(expected.get(index), Some(&(old_value, new_value)));
}

#[test]
fn cost_precheck_skips_post_execute_and_payment_calls_it_once_per_modifier() {
    COST_POST_EXECUTE_COUNT.store(0, Ordering::SeqCst);
    let mut app = test_app();
    let mana = register_attribute(&mut app, "Mana");
    let mut attributes = attribute_set(&app, mana, 100.0);
    attributes.set_post_execute(Some(count_cost_post_execute));
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle {
            attributes,
            ..Default::default()
        })
        .id();
    let ability = ability_with_cost_modifiers(vec![
        modifier(mana, ModifierOperation::Add, -20.0),
        modifier(mana, ModifierOperation::Add, -30.0),
    ]);

    assert!(can_activate_cost_ability(&mut app, source, &ability));
    assert_eq!(COST_POST_EXECUTE_COUNT.load(Ordering::SeqCst), 0);
    assert_eq!(
        cost_attribute_values(&mut app, source, mana),
        (100.0, 100.0)
    );
    assert_eq!(commit_cost_ability(&mut app, source, &ability), Ok(()));
    assert_eq!(COST_POST_EXECUTE_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(cost_attribute_values(&mut app, source, mana), (50.0, 50.0));
}

#[test]
fn cost_affordability_includes_effects_removed_before_payment() {
    let cases = [(10.0, 100.0, None), (100.0, -80.0, Some(50.0))];
    for (base, adjustment, expected) in cases {
        let mut app = test_app();
        let mana = register_attribute(&mut app, "Mana");
        let effect_tag = register_tag(&mut app, "Effect.ManaAdjustment");
        let granted_tag = register_tag(&mut app, "State.ManaAdjusted");
        let attributes = attribute_set(&app, mana, base);
        let source = app
            .world_mut()
            .spawn(GameplayAbilitySystemBundle {
                attributes,
                ..Default::default()
            })
            .id();
        let active_effect = Arc::new(GameplayEffect::new(
            vec![modifier(mana, ModifierOperation::Add, adjustment)],
            EffectDurationTicks::Infinite,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            effect_tags(vec![effect_tag], vec![granted_tag]),
        ));
        assert!(apply_effect(&mut app, source, source, active_effect));
        let handles_before = active_effect_handles(&app, source);
        let cost = Arc::new(GameplayEffect::new(
            vec![modifier(mana, ModifierOperation::Add, -50.0)],
            EffectDurationTicks::Instant,
            None,
            1.0,
            StackingPolicy::non_stacking(),
            EffectTags::new(Vec::new(), Vec::new()).with_remove_effects_with_tags(vec![effect_tag]),
        ));
        let ability = ability_with_cost_effect(cost);

        assert_eq!(
            can_activate_cost_ability(&mut app, source, &ability),
            expected.is_some()
        );
        assert_eq!(active_effect_handles(&app, source), handles_before);
        assert_eq!(
            cost_attribute_values(&mut app, source, mana),
            (base, base + adjustment)
        );
        assert!(
            app.world()
                .entity(source)
                .get::<GameplayTagContainer>()
                .unwrap()
                .has_tag(&granted_tag)
        );
        if let Some(expected) = expected {
            assert_eq!(commit_cost_ability(&mut app, source, &ability), Ok(()));
            assert_eq!(
                cost_attribute_values(&mut app, source, mana),
                (expected, expected)
            );
            assert!(active_effect_handles(&app, source).is_empty());
            assert!(
                !app.world()
                    .entity(source)
                    .get::<GameplayTagContainer>()
                    .unwrap()
                    .has_tag(&granted_tag)
            );
        } else {
            assert_cost_rejected(&mut app, source, &ability);
            assert_eq!(
                cost_attribute_values(&mut app, source, mana),
                (base, base + adjustment)
            );
            assert_eq!(active_effect_handles(&app, source), handles_before);
            assert!(
                app.world()
                    .entity(source)
                    .get::<GameplayTagContainer>()
                    .unwrap()
                    .has_tag(&granted_tag)
            );
        }
    }
}
