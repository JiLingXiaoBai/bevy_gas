use bevy_tools::UniqueNamePool;

#[test]
fn unique_name_pool_reuses_names_and_preserves_empty_name() {
    let mut pool = UniqueNamePool::default();
    let empty = pool.new_name("").unwrap();
    let first = pool.new_name("Ability.Fireball").unwrap();
    let second = pool.new_name("Ability.Fireball").unwrap();

    assert_eq!(Ok(empty), pool.new_name(""));
    assert_eq!(first, second);
    assert_eq!(pool.get_display_str(&empty), "");
    assert_eq!(pool.get_display_str(&first), "Ability.Fireball");

    pool.clear();
    assert_eq!(pool.get_display_str(&empty), "");
    assert_eq!(pool.new_name("Ability.Fireball"), Ok(first));
}

#[test]
fn unique_name_pool_keeps_distinct_names_separate() {
    let mut pool = UniqueNamePool::default();
    let names = ["First", "Second", "Ability.Fireball", "Effect.Fireball"];
    let handles = names.map(|name| pool.new_name(name).unwrap());

    for (index, handle) in handles.iter().enumerate() {
        assert_eq!(pool.get_display_str(handle), names[index]);
        assert_eq!(pool.new_name(names[index]), Ok(*handle));
        assert!(handles[index + 1..].iter().all(|other| other != handle));
    }
}
