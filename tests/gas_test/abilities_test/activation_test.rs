use super::*;
use crate::support_test::run_effect_duration_tick;
use bevy::ecs::system::SystemState;
use bevy_gas::{
    AbilityActivationCheckError, AbilityActivationCheckParams, AbilityActivationRequirementError,
    can_activate_ability,
};
use std::error::Error;

#[test]
fn ability_without_end_stays_active_and_disallows_multiple_instances() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        false,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    run_ability_tasks(&mut app);
    run_finished_ability_cleanup(&mut app);
    assert!(!activate_ability(&mut app, source, source, handle));
    assert_eq!(active_ability_count(&mut app), 1);
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

#[test]
fn ability_allows_multiple_instances_when_enabled() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::default(),
        Vec::new(),
        None,
        None,
        true,
    ));
    let handle = give_ability(&mut app, source, ability);

    assert!(activate_ability(&mut app, source, source, handle));
    assert!(activate_ability(&mut app, source, source, handle));
    assert_eq!(active_ability_count(&mut app), 2);
    assert_eq!(
        app.world()
            .entity(source)
            .get::<AbilitySystemComponent>()
            .unwrap()
            .find_ability_spec(handle)
            .unwrap()
            .get_active_count(),
        2
    );
}

#[test]
fn ability_activation_required_and_blocked_tags_are_enforced() {
    let mut app = test_app();
    let required = register_tag(&mut app, "State.Weapon.Ready");
    let blocked = register_tag(&mut app, "State.Silenced");
    let source = app
        .world_mut()
        .spawn((
            AbilitySystemComponent::default(),
            GameplayTagContainer::default(),
        ))
        .id();
    let ability = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![required],
            vec![blocked],
        ),
        Vec::new(),
        None,
        None,
        true,
    ));
    let handle = give_ability(&mut app, source, ability.clone());

    assert_activation_requirement_rejected(
        &mut app,
        source,
        handle,
        &ability,
        AbilityActivationRequirementError::MissingRequiredTags,
    );

    add_tag_to_entity(&mut app, source, required);
    assert!(activate_ability(&mut app, source, source, handle));

    add_tag_to_entity(&mut app, source, blocked);
    assert_activation_requirement_rejected(
        &mut app,
        source,
        handle,
        &ability,
        AbilityActivationRequirementError::ActivationBlocked,
    );
}

#[test]
fn active_ability_block_tags_prevent_matching_ability_activation() {
    let mut app = test_app();
    let channel_tag = register_tag(&mut app, "Ability.Channel");
    let movement_tag = register_tag(&mut app, "Ability.Movement");
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let channel = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![channel_tag],
            Vec::new(),
            vec![movement_tag],
            Vec::new(),
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        false,
    ));
    let movement = Arc::new(GameplayAbility::new(
        AbilityTags::new(
            vec![movement_tag],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Vec::new(),
        None,
        None,
        false,
    ));
    let channel_handle = give_ability(&mut app, source, channel);
    let movement_handle = give_ability(&mut app, source, movement.clone());

    assert!(activate_ability(&mut app, source, source, channel_handle));
    assert_activation_requirement_rejected(
        &mut app,
        source,
        movement_handle,
        &movement,
        AbilityActivationRequirementError::BlockedByAbility,
    );
}

#[test]
fn try_activate_ability_by_handle_returns_error_for_missing_spec() {
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn(AbilitySystemComponent::default())
        .id();
    let missing_handle = AbilitySpecHandle::new(999);

    let err = activate_ability_result(&mut app, source, source, missing_handle).unwrap_err();

    assert_eq!(
        err,
        AbilityActivationError::AbilityNotFound {
            source,
            handle: missing_handle
        }
    );
}

#[test]
fn cooldown_activation_requirement_matches_precheck_until_effect_expires() {
    let mut app = test_app();
    let cooldown_tag = register_tag(&mut app, "Cooldown.Precheck");
    let source = app
        .world_mut()
        .spawn(GameplayAbilitySystemBundle::default())
        .id();
    let cooldown = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::DurationTicks(ModifierMagnitude::Flat(2.0)),
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![cooldown_tag]),
    ));
    let ability = Arc::new(
        GameplayAbility::default()
            .with_cooldown(cooldown)
            .with_startup_tasks(vec![AbilityTaskDef::instant(
                AbilityTaskOnFinishedDef::EndAbility,
            )]),
    );
    let handle = give_ability(&mut app, source, ability.clone());
    let mut state = SystemState::<AbilityActivationCheckParams>::new(app.world_mut());
    assert_eq!(
        can_activate_ability(
            source,
            source,
            &ability,
            1,
            &state.get(app.world()).unwrap()
        ),
        Ok(())
    );
    assert!(activate_ability(&mut app, source, source, handle));
    run_finished_ability_cleanup(&mut app);
    assert_eq!(active_ability_count(&mut app), 0);
    assert!(
        app.world()
            .entity(source)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&cooldown_tag)
    );
    assert_activation_requirement_rejected(
        &mut app,
        source,
        handle,
        &ability,
        AbilityActivationRequirementError::CooldownActive,
    );

    run_effect_duration_tick(&mut app);
    assert_activation_requirement_rejected(
        &mut app,
        source,
        handle,
        &ability,
        AbilityActivationRequirementError::CooldownActive,
    );
    run_effect_duration_tick(&mut app);
    assert!(
        !app.world()
            .entity(source)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&cooldown_tag)
    );
    assert_eq!(
        can_activate_ability(
            source,
            source,
            &ability,
            1,
            &state.get(app.world()).unwrap()
        ),
        Ok(())
    );
    assert!(activate_ability(&mut app, source, source, handle));
}

fn assert_activation_requirement_rejected(
    app: &mut App,
    source: Entity,
    handle: AbilitySpecHandle,
    ability: &Arc<GameplayAbility>,
    expected: AbilityActivationRequirementError,
) {
    let mut state = SystemState::<AbilityActivationCheckParams>::new(app.world_mut());
    let precheck_error =
        can_activate_ability(source, source, ability, 1, &state.get(app.world()).unwrap())
            .unwrap_err();
    assert_eq!(
        precheck_error,
        AbilityActivationCheckError::Requirements(expected)
    );
    assert_eq!(
        precheck_error
            .source()
            .unwrap()
            .downcast_ref::<AbilityActivationRequirementError>(),
        Some(&expected)
    );

    let activation_error = activate_ability_result(app, source, source, handle).unwrap_err();
    assert_eq!(
        activation_error,
        AbilityActivationError::ActivationRequirementsNotMet {
            source,
            handle,
            error: expected,
        }
    );
    assert!(activation_error.is_rejection());
    assert!(activation_error.to_string().contains(&expected.to_string()));
    assert_eq!(
        activation_error
            .source()
            .unwrap()
            .downcast_ref::<AbilityActivationRequirementError>(),
        Some(&expected)
    );
}
