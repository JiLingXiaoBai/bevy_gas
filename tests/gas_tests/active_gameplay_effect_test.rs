use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_tools::attributes::{AttributeId, AttributeIdRegister, AttributeRegion, AttributeSet};
use bevy_tools::modifiers::{Modifier, ModifierMagnitude, ModifierOperation};
use bevy_tools::{
    AbilitySystemParams, EffectDurationTicks, EffectPayload, EffectTags,
    GameplayAbilitySystemPlugin, GameplayEffect, GameplayTag, GameplayTagContainer,
    GameplayTagManager, StackingPolicy, TagRequirements, UniqueNamePool, apply_gameplay_effect,
};
use std::sync::Arc;

#[derive(Resource)]
struct EffectUnderTest(Arc<GameplayEffect>);

#[derive(Resource)]
struct TargetUnderTest(Entity);

#[derive(Resource, Default)]
struct ApplyResult(bool);

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameplayAbilitySystemPlugin));
    app.init_resource::<ApplyResult>();
    app
}

fn register_tag(app: &mut App, name: &str) -> GameplayTag {
    let unique_name = {
        let mut names = app.world_mut().resource_mut::<UniqueNamePool>();
        names.new_name(name).unwrap()
    };

    app.world_mut()
        .resource_mut::<GameplayTagManager>()
        .register_tag_internal(unique_name, None)
        .unwrap()
}

fn register_attribute(app: &mut App, name: &str) -> AttributeId {
    let name = name.to_string();
    app.world_mut()
        .run_system_once(move |mut register: AttributeIdRegister| {
            register.request_or_register_attribute_id(&name, AttributeRegion::Cold)
        })
        .unwrap()
        .unwrap()
}

fn effect_tags(granted_tags: Vec<GameplayTag>) -> EffectTags {
    EffectTags::new(
        Vec::new(),
        granted_tags,
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        TagRequirements::default(),
        Vec::new(),
        Vec::new(),
    )
}

fn apply_effect_system(
    effect: Res<EffectUnderTest>,
    target: Res<TargetUnderTest>,
    mut params: AbilitySystemParams,
    mut result: ResMut<ApplyResult>,
) {
    let payload = EffectPayload::new(target.0, None, 1);
    result.0 = apply_gameplay_effect(target.0, &effect.0, &mut params, &payload).is_ok();
}

#[test]
fn tag_only_active_effect_does_not_require_attribute_set() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "State.Stunned");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();
    let effect = Arc::new(GameplayEffect::new(
        Vec::new(),
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![tag]),
    ));

    app.insert_resource(TargetUnderTest(target));
    app.insert_resource(EffectUnderTest(effect));
    app.world_mut()
        .run_system_once(apply_effect_system)
        .unwrap();

    assert!(app.world().resource::<ApplyResult>().0);
    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&tag)
    );
    assert!(app.world().entity(target).get::<AttributeSet>().is_none());
}

#[test]
fn failed_effect_application_does_not_leave_duration_modifier() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "State.Buffed");
    let health = register_attribute(&mut app, "Health");
    let manager = app
        .world()
        .resource::<bevy_tools::AttributeIdManager>()
        .clone();
    let mut attributes = AttributeSet::default();
    attributes
        .initialize_attribute(&manager, health, 10.0, None)
        .unwrap();
    let target = app.world_mut().spawn(attributes).id();
    let effect = Arc::new(GameplayEffect::new(
        vec![Modifier::new(
            health,
            ModifierOperation::Add,
            ModifierMagnitude::Flat(5.0),
        )],
        EffectDurationTicks::Infinite,
        None,
        1.0,
        StackingPolicy::non_stacking(),
        effect_tags(vec![tag]),
    ));

    app.insert_resource(TargetUnderTest(target));
    app.insert_resource(EffectUnderTest(effect));
    app.world_mut()
        .run_system_once(apply_effect_system)
        .unwrap();

    assert!(!app.world().resource::<ApplyResult>().0);
    let mut attributes = app.world_mut().entity_mut(target);
    let mut attributes = attributes.get_mut::<AttributeSet>().unwrap();
    assert_eq!(
        attributes.get_current_value(&manager, health),
        Ok(Some(10.0))
    );
}
