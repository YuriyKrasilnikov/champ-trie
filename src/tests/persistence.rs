use crate::ChampMap;

/// Checkpoint + insert + rollback = original state.
#[test]
fn rollback_after_insert() {
    let mut map = ChampMap::new();
    map.insert(1, 10);
    map.insert(2, 20);

    let cp = map.checkpoint();
    let saved_len = map.len();
    let saved_adhash = map.adhash();

    map.insert(3, 30);
    map.insert(4, 40);
    assert_eq!(map.len(), 4);

    map.rollback(cp);
    assert_eq!(map.len(), saved_len);
    assert_eq!(map.adhash(), saved_adhash);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), None);
    assert_eq!(map.get(&4), None);
}

/// Checkpoint + remove + rollback = original state.
#[test]
fn rollback_after_remove() {
    let mut map = ChampMap::new();
    map.insert("a", 1);
    map.insert("b", 2);

    let cp = map.checkpoint();

    map.remove(&"a");
    assert_eq!(map.len(), 1);

    map.rollback(cp);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&"a"), Some(&1));
    assert_eq!(map.get(&"b"), Some(&2));
}

/// Checkpoint on empty map + insert + rollback = empty.
#[test]
fn rollback_to_empty() {
    let mut map: ChampMap<i32, i32> = ChampMap::new();
    let cp = map.checkpoint();

    map.insert(1, 1);
    map.insert(2, 2);

    map.rollback(cp);
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

/// Multiple checkpoints: rollback to the earlier one.
#[test]
fn nested_checkpoints() {
    let mut map = ChampMap::new();
    map.insert(1, 10);
    let cp1 = map.checkpoint();

    map.insert(2, 20);
    let _cp2 = map.checkpoint();

    map.insert(3, 30);

    // Rollback to cp1 (before key 2 and 3 were added).
    map.rollback(cp1);
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), None);
}
