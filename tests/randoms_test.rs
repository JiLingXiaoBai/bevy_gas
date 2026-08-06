use bevy_tools::Random;

#[test]
fn random_with_same_seed_produces_same_sequence() {
    let mut first = Random::from_seed(42);
    let mut second = Random::from_seed(42);

    let first_values: Vec<u32> = (0..8).map(|_| first.random_range(0..1000)).collect();
    let second_values: Vec<u32> = (0..8).map(|_| second.random_range(0..1000)).collect();

    assert_eq!(first_values, second_values);
}

#[test]
fn random_bool_respects_extreme_probabilities() {
    let mut random = Random::from_seed(7);

    assert!(!random.random_bool(0.0));
    assert!(random.random_bool(1.0));
}
