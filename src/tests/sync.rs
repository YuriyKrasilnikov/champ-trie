use crate::ChampMapSync;

#[test]
fn sync_empty() {
    let map: ChampMapSync<i32, i32> = ChampMapSync::new();
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

#[test]
fn sync_insert_and_get() {
    let mut map = ChampMapSync::new();
    map.insert("key", 42);
    assert_eq!(map.get(&"key"), Some(&42));
    assert_eq!(map.len(), 1);
}

#[test]
fn sync_remove() {
    let mut map = ChampMapSync::new();
    map.insert(1, 10);
    map.insert(2, 20);
    assert!(map.remove(&1));
    assert_eq!(map.get(&1), None);
    assert_eq!(map.len(), 1);
}

#[test]
fn sync_canonical_order() {
    let mut m1 = ChampMapSync::new();
    m1.insert(1, 10);
    m1.insert(2, 20);
    m1.insert(3, 30);

    let mut m2 = ChampMapSync::new();
    m2.insert(3, 30);
    m2.insert(1, 10);
    m2.insert(2, 20);

    assert_eq!(m1.adhash(), m2.adhash());
}

#[test]
fn sync_checkpoint_rollback() {
    let mut map = ChampMapSync::new();
    map.insert(1, 10);
    let cp = map.checkpoint();

    map.insert(2, 20);
    map.rollback(cp);

    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), None);
}

#[test]
fn sync_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ChampMapSync<String, i32>>();
}

#[test]
fn sync_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<ChampMapSync<String, i32>>();
}

#[test]
fn sync_stress_100() {
    let mut map = ChampMapSync::new();
    for i in 0_u64..100 {
        map.insert(i, i * 5);
    }
    assert_eq!(map.len(), 100);
    for i in 0_u64..100 {
        assert_eq!(map.get(&i), Some(&(i * 5)));
    }
}
