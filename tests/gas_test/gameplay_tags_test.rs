use super::support_test::{add_tag_to_entity, register_tag, remove_tag_from_entity, test_app};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_tools::{
    GameplayAbilitySystemSettings, GameplayTag, GameplayTagContainer, GameplayTagError,
    GameplayTagManager, GameplayTagRegister, TagRequirements, UniqueNamePool,
};

fn inherited_bits_contain(
    manager: &GameplayTagManager,
    tag: GameplayTag,
    expected: GameplayTag,
) -> bool {
    let Ok(bits) = manager.get_inherited_bits(&tag) else {
        return false;
    };
    let index = expected.get_bit_index_usize();
    let block = index / 64;
    let bit = index % 64;
    (bits[block] & (1u64 << bit)) != 0
}

#[test]
fn child_tag_sets_parent_bits_and_removal_respects_ref_counts() {
    let mut app = test_app();
    let stun = register_tag(&mut app, "Effect.Debuff.Stun");
    let slow = register_tag(&mut app, "Effect.Debuff.Slow");
    let debuff = register_tag(&mut app, "Effect.Debuff");
    let effect = register_tag(&mut app, "Effect");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();

    let manager = app.world().resource::<GameplayTagManager>();
    assert!(inherited_bits_contain(manager, stun, debuff));
    assert!(inherited_bits_contain(manager, stun, effect));

    add_tag_to_entity(&mut app, target, stun);
    add_tag_to_entity(&mut app, target, slow);

    let tags = app
        .world()
        .entity(target)
        .get::<GameplayTagContainer>()
        .unwrap();
    assert!(tags.has_tag(&stun));
    assert!(tags.has_tag(&slow));
    assert!(tags.has_tag(&debuff));
    assert!(tags.has_tag(&effect));

    remove_tag_from_entity(&mut app, target, stun);
    let tags = app
        .world()
        .entity(target)
        .get::<GameplayTagContainer>()
        .unwrap();
    assert!(!tags.has_tag(&stun));
    assert!(tags.has_tag(&slow));
    assert!(tags.has_tag(&debuff));
    assert!(tags.has_tag(&effect));

    remove_tag_from_entity(&mut app, target, slow);
    let tags = app
        .world()
        .entity(target)
        .get::<GameplayTagContainer>()
        .unwrap();
    assert!(!tags.has_tag(&slow));
    assert!(!tags.has_tag(&debuff));
    assert!(!tags.has_tag(&effect));
}

#[test]
fn tag_requirements_match_inherited_bits_and_ignored_tags() {
    let mut app = test_app();
    let stun = register_tag(&mut app, "Effect.Debuff.Stun");
    let debuff = register_tag(&mut app, "Effect.Debuff");
    let buff = register_tag(&mut app, "Effect.Buff");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();

    add_tag_to_entity(&mut app, target, stun);

    let tags = app
        .world()
        .entity(target)
        .get::<GameplayTagContainer>()
        .unwrap();
    assert!(
        TagRequirements::new(vec![debuff], Vec::new())
            .unwrap()
            .passes(Some(tags))
    );
    assert!(
        !TagRequirements::new(vec![buff], Vec::new())
            .unwrap()
            .passes(Some(tags))
    );
    assert!(
        !TagRequirements::new(Vec::new(), vec![debuff])
            .unwrap()
            .passes(Some(tags))
    );
}

#[test]
fn repeated_tag_registration_returns_existing_tag() {
    let mut app = test_app();
    let first = register_tag(&mut app, "Ability.Fireball");
    let second = register_tag(&mut app, "Ability.Fireball");
    let parent = register_tag(&mut app, "Ability");

    assert_eq!(first, second);
    assert!(inherited_bits_contain(
        app.world().resource::<GameplayTagManager>(),
        first,
        parent
    ));
}

#[test]
fn empty_tag_requirements_pass_without_container() {
    let requirements = TagRequirements::default();

    assert!(requirements.passes(None));
}

#[test]
fn non_empty_tag_requirements_fail_without_container() {
    let mut app = test_app();
    let required = register_tag(&mut app, "State.Ready");
    let ignored = register_tag(&mut app, "State.Silenced");

    assert!(
        !TagRequirements::new(vec![required], Vec::new())
            .unwrap()
            .passes(None)
    );
    assert!(
        !TagRequirements::new(Vec::new(), vec![ignored])
            .unwrap()
            .passes(None)
    );
}

#[test]
fn duplicate_adds_require_matching_removes_before_bit_clears() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "State.Rooted");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();

    add_tag_to_entity(&mut app, target, tag);
    add_tag_to_entity(&mut app, target, tag);
    remove_tag_from_entity(&mut app, target, tag);

    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&tag)
    );

    remove_tag_from_entity(&mut app, target, tag);
    assert!(
        !app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&tag)
    );
}

#[test]
fn removing_unheld_tag_does_not_clear_other_tags() {
    let mut app = test_app();
    let held = register_tag(&mut app, "State.Hasted");
    let missing = register_tag(&mut app, "State.Stunned");
    let target = app.world_mut().spawn(GameplayTagContainer::default()).id();

    add_tag_to_entity(&mut app, target, held);
    remove_tag_from_entity(&mut app, target, missing);

    assert!(
        app.world()
            .entity(target)
            .get::<GameplayTagContainer>()
            .unwrap()
            .has_tag(&held)
    );
}

#[test]
fn tag_registration_reports_capacity_exceeded() {
    let mut app = test_app();

    let result = app
        .world_mut()
        .run_system_once(|mut register: GameplayTagRegister| {
            let mut final_result = Ok(());
            for index in 0..=GameplayAbilitySystemSettings::GAMEPLAY_TAG_SIZE {
                let tag_name = format!("Tag{index}");
                if let Err(err) = register.request_or_register_tag(&tag_name) {
                    final_result = Err(err);
                    break;
                }
            }
            final_result
        })
        .unwrap();

    assert_eq!(
        result,
        Err(GameplayTagError::CapacityExceeded {
            max: GameplayAbilitySystemSettings::GAMEPLAY_TAG_SIZE
        })
    );
}

#[test]
fn inherited_bits_report_manager_mismatch() {
    let mut app = test_app();
    let tag = register_tag(&mut app, "State.Ready");
    let empty_manager = GameplayTagManager::default();

    assert_eq!(
        empty_manager.get_inherited_bits(&tag),
        Err(GameplayTagError::InvalidTagIndex {
            index: tag.get_bit_index_usize(),
        })
    );
}

#[test]
fn tag_registration_rejects_invalid_parent_index() {
    let mut app = test_app();
    let name = app
        .world_mut()
        .resource_mut::<UniqueNamePool>()
        .new_name("State.InvalidChild")
        .unwrap();
    let result = app
        .world_mut()
        .resource_mut::<GameplayTagManager>()
        .register_tag_internal(name, Some(u16::MAX));

    assert_eq!(
        result,
        Err(GameplayTagError::InvalidTagIndex {
            index: u16::MAX as usize,
        })
    );
}
