use crate::ChampMap;

#[test]
fn default_is_empty() {
    let map: ChampMap<i32, i32> = ChampMap::default();
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

#[test]
fn debug_format() {
    let map: ChampMap<i32, i32> = ChampMap::new();
    let dbg = format!("{map:?}");
    assert!(dbg.contains("ChampMap"));
    assert!(dbg.contains("len"));
}

#[test]
fn from_iterator() {
    let map: ChampMap<i32, i32> = vec![(1, 10), (2, 20), (3, 30)].into_iter().collect();
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&1), Some(&10));
}

#[test]
fn extend_trait() {
    let mut map = ChampMap::new();
    map.insert(1, 10);
    map.extend(vec![(2, 20), (3, 30)]);
    assert_eq!(map.len(), 3);
}

#[test]
fn index_existing() {
    let mut map = ChampMap::new();
    map.insert("key", 42);
    assert_eq!(map[&"key"], 42);
}

#[test]
#[should_panic(expected = "key not found")]
fn index_missing_panics() {
    let map: ChampMap<i32, i32> = ChampMap::new();
    let _ = map[&999];
}
