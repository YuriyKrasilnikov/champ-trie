use crate::ChampMap;

#[test]
fn empty_map() {
    let map: ChampMap<String, i32> = ChampMap::new();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

#[test]
fn insert_one() {
    let mut map = ChampMap::new();
    let old = map.insert("hello", 42);
    assert_eq!(old, None);
    assert_eq!(map.len(), 1);
    assert!(!map.is_empty());
    assert_ne!(map.adhash(), 0);
}

#[test]
fn insert_and_get() {
    let mut map = ChampMap::new();
    map.insert("key", 100);
    assert_eq!(map.get(&"key"), Some(&100));
}

#[test]
fn get_missing_key() {
    let mut map = ChampMap::new();
    map.insert("a", 1);
    assert_eq!(map.get(&"b"), None);
}

#[test]
fn insert_multiple() {
    let mut map = ChampMap::new();
    for i in 0..10 {
        map.insert(i, i * 10);
    }
    assert_eq!(map.len(), 10);
    for i in 0..10 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
}

#[test]
fn overwrite_value() {
    let mut map = ChampMap::new();
    assert_eq!(map.insert("k", 1), None);
    assert_eq!(map.insert("k", 2), Some(1));
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&"k"), Some(&2));
}

#[test]
fn contains_key_true() {
    let mut map = ChampMap::new();
    map.insert(42, "val");
    assert!(map.contains_key(&42));
}

#[test]
fn contains_key_false() {
    let mut map = ChampMap::new();
    map.insert(1, "a");
    assert!(!map.contains_key(&2));
}

#[test]
fn remove_existing() {
    let mut map = ChampMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    assert_eq!(map.remove(&"a"), Some(1));
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&"a"), None);
    assert_eq!(map.get(&"b"), Some(&2));
}

#[test]
fn remove_missing() {
    let mut map = ChampMap::new();
    map.insert("a", 1);
    assert_eq!(map.remove(&"z"), None);
    assert_eq!(map.len(), 1);
}

#[test]
fn remove_all() {
    let mut map = ChampMap::new();
    map.insert(1, 10);
    map.insert(2, 20);
    map.insert(3, 30);
    assert_eq!(map.remove(&1), Some(10));
    assert_eq!(map.remove(&2), Some(20));
    assert_eq!(map.remove(&3), Some(30));
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

#[test]
fn adhash_changes_on_insert() {
    let mut map = ChampMap::new();
    let h0 = map.adhash();
    map.insert(1, 1);
    let h1 = map.adhash();
    map.insert(2, 2);
    let h2 = map.adhash();
    assert_ne!(h0, h1);
    assert_ne!(h1, h2);
}

#[test]
fn adhash_changes_on_overwrite() {
    let mut map = ChampMap::new();
    map.insert("k", 1);
    let h1 = map.adhash();
    map.insert("k", 2);
    let h2 = map.adhash();
    assert_ne!(h1, h2);
}
