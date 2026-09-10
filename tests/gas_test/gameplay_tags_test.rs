use super::support_test::{add_tag_to_entity, register_tag, remove_tag_from_entity, test_app};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_gas::{
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

#[test]
fn removing_inherited_ancestors_preserves_descendant_references() {
    let mut app = test_app();
    let child = register_tag(&mut app, "State.CrowdControl.Stunned");
    let parent = register_tag(&mut app, "State.CrowdControl");
    let root = register_tag(&mut app, "State");
    let manager = app.world().resource::<GameplayTagManager>();
    let mut tags = GameplayTagContainer::default();

    tags.add_tag(&child, manager).unwrap();
    tags.remove_tag(&parent, manager).unwrap();
    tags.remove_tag(&root, manager).unwrap();
    tags.remove_tags(&[root, parent, parent], manager).unwrap();
    assert!(tags.has_all(&[root, parent, child]));

    tags.remove_tag(&child, manager).unwrap();
    assert!(!tags.has_any(&[root, parent, child]));
}

#[test]
fn mixed_removals_consume_only_explicit_parent_and_child_references() {
    let mut app = test_app();
    let child = register_tag(&mut app, "State.CrowdControl.Stunned");
    let parent = register_tag(&mut app, "State.CrowdControl");
    let root = register_tag(&mut app, "State");
    let manager = app.world().resource::<GameplayTagManager>();
    let mut tags = GameplayTagContainer::default();

    tags.add_tags(&[root, parent, parent, child, child], manager)
        .unwrap();
    tags.remove_tag(&parent, manager).unwrap();
    tags.remove_tags(&[root, parent, parent, root], manager)
        .unwrap();
    assert!(tags.has_all(&[root, parent, child]));

    tags.remove_tag(&child, manager).unwrap();
    assert!(tags.has_all(&[root, parent, child]));

    tags.remove_tags(&[parent, child, parent, root], manager)
        .unwrap();
    assert!(!tags.has_any(&[root, parent, child]));
}

#[test]
fn parent_and_child_references_remain_independent_across_bit_blocks() {
    let mut app = test_app();
    for index in 0..63 {
        register_tag(&mut app, &format!("Padding{index}"));
    }
    let child = register_tag(&mut app, "Boundary.Child");
    let parent = register_tag(&mut app, "Boundary");
    assert_eq!(parent.get_bit_index_usize(), 63);
    assert_eq!(child.get_bit_index_usize(), 64);
    let manager = app.world().resource::<GameplayTagManager>();
    let mut tags = GameplayTagContainer::default();

    tags.add_tag(&child, manager).unwrap();
    tags.remove_tag(&parent, manager).unwrap();
    assert!(tags.has_all(&[parent, child]));

    tags.add_tag(&parent, manager).unwrap();
    tags.remove_tags(&[parent, parent], manager).unwrap();
    assert!(tags.has_all(&[parent, child]));

    tags.remove_tag(&child, manager).unwrap();
    assert!(!tags.has_any(&[parent, child]));
}

#[test]
fn single_tag_reference_overflow_preserves_existing_references() {
    let mut app = test_app();
    let child = register_tag(&mut app, "State.Ready");
    let sibling = register_tag(&mut app, "State.Running");
    let parent = register_tag(&mut app, "State");
    let manager = app.world().resource::<GameplayTagManager>();
    let mut tags = GameplayTagContainer::default();

    for _ in 0..u16::MAX {
        tags.add_tag(&child, manager).unwrap();
    }
    for rejected_tag in [parent, child, sibling] {
        assert_eq!(
            tags.add_tag(&rejected_tag, manager),
            Err(GameplayTagError::ReferenceCountOverflow {
                index: parent.get_bit_index_usize(),
                max: u16::MAX,
            })
        );
    }
    assert!(tags.has_all(&[parent, child]));
    assert!(!tags.has_tag(&sibling));

    tags.remove_tag(&parent, manager).unwrap();
    tags.remove_tag(&child, manager).unwrap();
    tags.add_tag(&sibling, manager).unwrap();
    for _ in 0..u16::MAX - 1 {
        tags.remove_tag(&child, manager).unwrap();
    }
    assert!(!tags.has_tag(&child));
    assert!(tags.has_all(&[parent, sibling]));

    tags.remove_tag(&sibling, manager).unwrap();
    assert!(!tags.has_any(&[parent, child, sibling]));
}

#[test]
fn batch_reference_overflow_is_atomic_for_duplicates_and_shared_ancestors() {
    let mut app = test_app();
    let child = register_tag(&mut app, "State.Ready");
    let sibling = register_tag(&mut app, "State.Running");
    let parent = register_tag(&mut app, "State");
    let unrelated = register_tag(&mut app, "Unrelated");
    let manager = app.world().resource::<GameplayTagManager>();
    let mut tags = GameplayTagContainer::default();

    for _ in 0..u16::MAX - 1 {
        tags.add_tag(&child, manager).unwrap();
    }
    for rejected_batch in [
        [unrelated, parent, parent],
        [unrelated, sibling, sibling],
        [unrelated, sibling, child],
    ] {
        assert_eq!(
            tags.add_tags(&rejected_batch, manager),
            Err(GameplayTagError::ReferenceCountOverflow {
                index: parent.get_bit_index_usize(),
                max: u16::MAX,
            })
        );
        assert!(tags.has_all(&[parent, child]));
        assert!(!tags.has_any(&[sibling, unrelated]));
    }

    tags.remove_tag(&parent, manager).unwrap();
    for _ in 0..u16::MAX - 1 {
        tags.remove_tag(&child, manager).unwrap();
    }
    assert!(!tags.has_any(&[parent, child, sibling, unrelated]));

    let oversized_batch = vec![child; usize::from(u16::MAX) + 1];
    assert_eq!(
        tags.add_tags(&oversized_batch, manager),
        Err(GameplayTagError::ReferenceCountOverflow {
            index: parent.get_bit_index_usize(),
            max: u16::MAX,
        })
    );
    assert!(!tags.has_any(&[parent, child, sibling, unrelated]));

    tags.add_tags(&[parent, child, sibling, unrelated], manager)
        .unwrap();
    tags.remove_tags(&[child, parent, parent, sibling, unrelated], manager)
        .unwrap();
    assert!(!tags.has_any(&[parent, child, sibling, unrelated]));
}
