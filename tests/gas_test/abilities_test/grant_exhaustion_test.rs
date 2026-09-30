//! Private unit tests for ability-grant exhaustion.
//! Loaded by the component module to exercise private boundary state.

use super::{AbilityGrantError, AbilitySystemComponent, GameplayAbility};
use std::sync::Arc;

#[test]
fn final_handles_preserve_existing_ability_identity() {
    let mut abilities = AbilitySystemComponent::default();
    let original = Arc::new(GameplayAbility::default());
    let original_handle = abilities.give_ability(Arc::clone(&original), 1).unwrap();
    abilities.next_ability_handle = u64::from(u32::MAX - 1);
    let definition = Arc::new(GameplayAbility::default());
    let penultimate = abilities.give_ability(Arc::clone(&definition), 2).unwrap();
    let final_handle = abilities.give_ability(Arc::clone(&definition), 3).unwrap();

    assert_eq!(original_handle.get_value(), 0);
    assert_eq!(penultimate.get_value(), u32::MAX - 1);
    assert_eq!(final_handle.get_value(), u32::MAX);
    assert_eq!(
        abilities.give_ability(Arc::clone(&definition), 4),
        Err(AbilityGrantError::HandleExhausted)
    );
    assert_eq!(abilities.get_ability_specs().len(), 3);
    assert!(Arc::ptr_eq(
        abilities
            .find_ability_spec(original_handle)
            .unwrap()
            .get_ability(),
        &original,
    ));
    assert_eq!(
        abilities
            .find_ability_spec(penultimate)
            .unwrap()
            .get_level(),
        2
    );
    assert_eq!(
        abilities
            .find_ability_spec(final_handle)
            .unwrap()
            .get_level(),
        3
    );
    assert!(abilities.clear_ability(penultimate));
    assert_eq!(abilities.get_ability_specs().len(), 2);
    assert!(abilities.find_ability_spec(original_handle).is_some());
    assert!(abilities.find_ability_spec(final_handle).is_some());
    assert_eq!(
        abilities.give_ability(definition, 5),
        Err(AbilityGrantError::HandleExhausted)
    );
}

#[test]
fn exhausted_grants_leave_active_specs_and_indices_unchanged() {
    let mut abilities = AbilitySystemComponent::default();
    let definition = Arc::new(GameplayAbility::default());
    let handle = abilities.give_ability(Arc::clone(&definition), 7).unwrap();
    abilities
        .find_ability_spec_mut(handle)
        .unwrap()
        .increment_active_count();
    abilities.next_ability_handle = u64::from(u32::MAX) + 1;
    let indices = abilities.ability_indices.clone();

    for _ in 0..2 {
        assert_eq!(
            abilities.give_ability(Arc::clone(&definition), 99),
            Err(AbilityGrantError::HandleExhausted)
        );
        assert_eq!(abilities.next_ability_handle, u64::from(u32::MAX) + 1);
        assert_eq!(abilities.ability_indices, indices);
        assert_eq!(abilities.get_ability_specs().len(), 1);
        let spec = abilities.find_ability_spec(handle).unwrap();
        assert_eq!(spec.get_level(), 7);
        assert_eq!(spec.get_active_count(), 1);
        assert!(Arc::ptr_eq(spec.get_ability(), &definition));
        assert!(!abilities.clear_ability(handle));
    }
}
