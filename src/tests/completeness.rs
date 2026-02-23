//! A₃ Completeness tests: insert/remove must not lose data.

use crate::ChampMap;

// ---------------------------------------------------------------------------
// insert must return the old value when updating
// ---------------------------------------------------------------------------

#[test]
fn insert_new_returns_none() {
    let mut map = ChampMap::new();
    let old = map.insert("key", 42);
    assert_eq!(old, None, "inserting new key should return None");
}

#[test]
fn insert_update_returns_old_value() {
    let mut map = ChampMap::new();
    map.insert("key", 1);
    let old = map.insert("key", 2);
    assert_eq!(old, Some(1), "updating should return the previous value");
}

#[test]
fn insert_update_chain() {
    let mut map = ChampMap::new();
    assert_eq!(map.insert("k", 10), None);
    assert_eq!(map.insert("k", 20), Some(10));
    assert_eq!(map.insert("k", 30), Some(20));
    assert_eq!(map.get(&"k"), Some(&30));
}

// ---------------------------------------------------------------------------
// remove must return the removed value
// ---------------------------------------------------------------------------

#[test]
fn remove_existing_returns_value() {
    let mut map = ChampMap::new();
    map.insert("a", 100);
    let removed = map.remove(&"a");
    assert_eq!(removed, Some(100), "remove should return the removed value");
}

#[test]
fn remove_missing_returns_none() {
    let mut map = ChampMap::new();
    map.insert("a", 1);
    let removed = map.remove(&"z");
    assert_eq!(removed, None, "removing missing key should return None");
}

#[test]
fn remove_returns_correct_value_among_many() {
    let mut map = ChampMap::new();
    for i in 0..100 {
        map.insert(i, i * 10);
    }
    assert_eq!(map.remove(&50), Some(500));
    assert_eq!(map.remove(&50), None);
    assert_eq!(map.len(), 99);
}
