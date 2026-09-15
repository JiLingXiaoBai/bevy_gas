use super::*;
use bevy::prelude::{App, Entity};
use bevy_gas::{
    ActiveEffectHandle, AttributeId, EffectSystemParams, GameplayTagManager, ModifierSourceId,
    ModifierSpec, remove_active_effect,
};

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
    let replacing_effect_tags =
        EffectTags::new(Vec::new(), Vec::new()).with_remove_effects_with_tags(vec![buff_tag]);
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
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
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
        EffectTags::new(Vec::new(), Vec::new()).with_remove_effects_with_tags(vec![effect_tag]),
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
        .spawn((
            GameplayTagContainer::default(),
            attributes,
            ActiveGameplayEffects::default(),
        ))
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
        EffectTags::new(Vec::new(), Vec::new()).with_remove_effects_with_tags(vec![buff_tag]),
    ));
    {
        let mut queue = app.world_mut().resource_mut::<GameplayExecutionQueue>();
        queue
            .push_application(target, buff, EffectPayload::new(target, None, 1))
            .unwrap();
        queue
            .push_application(target, cleanse, EffectPayload::new(target, None, 1))
            .unwrap();
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
fn removing_effect_container_cleans_owned_contributions() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let granted_tag = register_tag(&mut app, "State.Buffed");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    app.world_mut()
        .entity_mut(target)
        .insert(GameplayTagContainer::default());
    add_tag_to_entity(&mut app, target, granted_tag);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted_tag]),
    ));
    assert!(apply_effect(&mut app, target, target, effect));
    assert_eq!(current_value(&mut app, target, power), 110.0);

    app.world_mut()
        .entity_mut(target)
        .remove::<ActiveGameplayEffects>();
    app.world_mut().flush();

    assert_eq!(current_value(&mut app, target, power), 100.0);
    assert!(
        app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
    remove_tag_from_entity(&mut app, target, granted_tag);
    assert!(
        !app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
}

#[test]
fn replacing_effect_container_retires_handles_and_preserves_new_contributions() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let granted_tag = register_tag(&mut app, "State.Buffed");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    app.world_mut()
        .entity_mut(target)
        .insert(GameplayTagContainer::default());
    let old_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted_tag]),
    ));
    assert!(apply_effect(&mut app, target, target, old_effect));
    let old_handle = active_effect_handles(&app, target)[0];

    app.world_mut()
        .entity_mut(target)
        .insert(ActiveGameplayEffects::default());
    let new_effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 20.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted_tag]),
    ));
    assert!(apply_effect(&mut app, target, target, new_effect));
    app.world_mut().flush();

    let new_handle = active_effect_handles(&app, target)[0];
    assert_eq!(old_handle.get_slot(), new_handle.get_slot());
    assert_eq!(old_handle.get_generation(), new_handle.get_generation());
    assert_ne!(old_handle.get_storage_id(), new_handle.get_storage_id());
    assert!(
        app.world()
            .get::<ActiveGameplayEffects>(target)
            .unwrap()
            .get(old_handle)
            .is_none()
    );
    assert_eq!(current_value(&mut app, target, power), 120.0);
    assert!(
        app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
    assert!(
        app.world_mut()
            .run_system_once(move |mut params: EffectSystemParams| {
                remove_active_effect(new_handle, &mut params).unwrap()
            })
            .unwrap()
    );
    assert_eq!(current_value(&mut app, target, power), 100.0);
    assert!(
        !app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
}

#[test]
fn removing_inhibited_container_preserves_foreign_tag_references() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let enabled = register_tag(&mut app, "State.Enabled");
    let granted_tag = register_tag(&mut app, "State.Buffed");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    app.world_mut()
        .entity_mut(target)
        .insert(GameplayTagContainer::default());
    add_tag_to_entity(&mut app, target, enabled);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        tags_with_requirements(
            vec![granted_tag],
            TagRequirements::new(vec![enabled], Vec::new()).unwrap(),
            TagRequirements::default(),
        ),
    ));
    assert!(apply_effect(&mut app, target, target, effect));
    remove_tag_from_entity(&mut app, target, enabled);
    run_effect_tag_requirements_update(&mut app);
    add_tag_to_entity(&mut app, target, granted_tag);

    app.world_mut()
        .entity_mut(target)
        .remove::<ActiveGameplayEffects>();
    app.world_mut().flush();

    assert_eq!(current_value(&mut app, target, power), 100.0);
    assert!(
        app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
    remove_tag_from_entity(&mut app, target, granted_tag);
    assert!(
        !app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
}

#[test]
fn replacing_tags_and_effects_together_preserves_new_tag_references() {
    let mut app = test_app();
    let granted_tag = register_tag(&mut app, "State.Buffed");
    let target = app
        .world_mut()
        .spawn((
            GameplayTagContainer::default(),
            ActiveGameplayEffects::default(),
        ))
        .id();
    let old_effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(Vec::new(), vec![granted_tag]),
    ));
    assert!(apply_effect(&mut app, target, target, old_effect));
    let mut replacement_tags = GameplayTagContainer::default();
    replacement_tags
        .add_tag(&granted_tag, app.world().resource::<GameplayTagManager>())
        .unwrap();

    app.world_mut()
        .entity_mut(target)
        .insert((replacement_tags, ActiveGameplayEffects::default()));

    assert!(active_effect_handles(&app, target).is_empty());
    assert!(
        app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
    remove_tag_from_entity(&mut app, target, granted_tag);
    assert!(
        !app.world()
            .get::<GameplayTagContainer>(target)
            .unwrap()
            .has_tag(&granted_tag)
    );
}

fn install_caller_modifier_matching_effect(
    app: &mut App,
    target: Entity,
    attribute: AttributeId,
    effect_handle: ActiveEffectHandle,
) -> ModifierSourceId {
    let caller_source = ModifierSourceId::new(
        effect_handle.get_storage_id(),
        effect_handle.get_slot(),
        effect_handle.get_generation(),
    );
    let runtime_source = ModifierSourceId::from(effect_handle);
    assert_eq!(caller_source.get_scope(), runtime_source.get_scope());
    assert_eq!(caller_source.get_slot(), runtime_source.get_slot());
    assert_eq!(
        caller_source.get_generation(),
        runtime_source.get_generation()
    );
    assert_ne!(caller_source, runtime_source);
    app.world_mut()
        .run_system_once(move |mut params: EffectSystemParams| {
            params
                .attr_set_query
                .get_mut(target)
                .unwrap()
                .apply_duration_modifier(
                    &params.attribute_id_manager,
                    &ModifierSpec::new(attribute, ModifierOperation::Add, 7.0),
                    caller_source,
                )
                .unwrap();
        })
        .unwrap();
    caller_source
}

#[test]
fn removing_effect_preserves_caller_modifier_with_matching_numeric_identity() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    assert!(apply_effect(&mut app, target, target, effect));
    let handle = active_effect_handles(&app, target)[0];
    let caller_source = install_caller_modifier_matching_effect(&mut app, target, power, handle);
    assert_eq!(current_value(&mut app, target, power), 117.0);

    assert!(
        app.world_mut()
            .run_system_once(move |mut params: EffectSystemParams| {
                remove_active_effect(handle, &mut params).unwrap()
            })
            .unwrap()
    );

    assert_eq!(current_value(&mut app, target, power), 107.0);
    app.world_mut()
        .get_mut::<AttributeSet>(target)
        .unwrap()
        .remove_modifiers(caller_source);
    assert_eq!(current_value(&mut app, target, power), 100.0);
}

#[test]
fn replacing_effect_container_preserves_caller_modifier_with_matching_numeric_identity() {
    let mut app = test_app();
    let power = register_attribute(&mut app, "Power");
    let target = spawn_attribute_set(&mut app, power, 100.0);
    let effect = Arc::new(GameplayEffect::new(
        vec![add_modifier(power, 10.0)],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        empty_effect_tags(),
    ));
    assert!(apply_effect(&mut app, target, target, Arc::clone(&effect)));
    let old_handle = active_effect_handles(&app, target)[0];
    let caller_source =
        install_caller_modifier_matching_effect(&mut app, target, power, old_handle);

    app.world_mut()
        .entity_mut(target)
        .insert(ActiveGameplayEffects::default());

    assert_eq!(current_value(&mut app, target, power), 107.0);
    assert!(apply_effect(&mut app, target, target, effect));
    let new_handle = active_effect_handles(&app, target)[0];
    assert_ne!(old_handle.get_storage_id(), new_handle.get_storage_id());
    assert!(
        app.world()
            .get::<ActiveGameplayEffects>(target)
            .unwrap()
            .get(old_handle)
            .is_none()
    );
    assert_eq!(current_value(&mut app, target, power), 117.0);
    app.world_mut()
        .get_mut::<AttributeSet>(target)
        .unwrap()
        .remove_modifiers(caller_source);
    assert_eq!(current_value(&mut app, target, power), 110.0);
}
