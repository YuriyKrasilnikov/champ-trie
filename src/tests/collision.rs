use std::hash::{Hash, Hasher};

use crate::ChampMap;

/// A key type with a controllable hash value for testing hash collisions.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CollidingKey {
    id: u32,
    forced_hash: u64,
}

impl CollidingKey {
    const fn new(id: u32, hash: u64) -> Self {
        Self {
            id,
            forced_hash: hash,
        }
    }
}

impl Hash for CollidingKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.forced_hash.hash(state);
    }
}

/// Two keys with the same 64-bit hash create a collision node.
#[test]
fn two_colliding_keys() {
    let k1 = CollidingKey::new(1, 0xDEAD_BEEF);
    let k2 = CollidingKey::new(2, 0xDEAD_BEEF);

    let mut map = ChampMap::new();
    map.insert(k1.clone(), "first");
    map.insert(k2.clone(), "second");

    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&k1), Some(&"first"));
    assert_eq!(map.get(&k2), Some(&"second"));
}

/// Three keys with the same hash.
#[test]
fn three_colliding_keys() {
    let keys: Vec<CollidingKey> = (0..3).map(|i| CollidingKey::new(i, 0xCAFE)).collect();

    let mut map = ChampMap::new();
    for (i, k) in keys.iter().enumerate() {
        map.insert(k.clone(), i);
    }

    assert_eq!(map.len(), 3);
    for (i, k) in keys.iter().enumerate() {
        assert_eq!(map.get(k), Some(&i));
    }
}

/// Remove from collision node.
#[test]
fn remove_from_collision() {
    let k1 = CollidingKey::new(1, 0xAAAA);
    let k2 = CollidingKey::new(2, 0xAAAA);
    let k3 = CollidingKey::new(3, 0xAAAA);

    let mut map = ChampMap::new();
    map.insert(k1.clone(), 10);
    map.insert(k2.clone(), 20);
    map.insert(k3.clone(), 30);

    assert!(map.remove(&k2));
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&k1), Some(&10));
    assert_eq!(map.get(&k2), None);
    assert_eq!(map.get(&k3), Some(&30));
}

/// Overwrite in collision node.
#[test]
fn overwrite_in_collision() {
    let k1 = CollidingKey::new(1, 0xBBBB);
    let k2 = CollidingKey::new(2, 0xBBBB);

    let mut map = ChampMap::new();
    map.insert(k1.clone(), "old");
    map.insert(k2.clone(), "val2");
    map.insert(k1.clone(), "new");

    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&k1), Some(&"new"));
}

/// Collision node with remove-all returns to empty.
#[test]
fn collision_remove_all() {
    let k1 = CollidingKey::new(1, 0xCCCC);
    let k2 = CollidingKey::new(2, 0xCCCC);

    let mut map = ChampMap::new();
    map.insert(k1.clone(), 1);
    map.insert(k2.clone(), 2);

    map.remove(&k1);
    map.remove(&k2);
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

/// Mixed: some keys collide, some don't.
#[test]
fn mixed_collisions_and_normal() {
    let collide_a = CollidingKey::new(1, 0xDDDD);
    let collide_b = CollidingKey::new(2, 0xDDDD);
    let normal = CollidingKey::new(3, 0xEEEE);

    let mut map = ChampMap::new();
    map.insert(collide_a.clone(), "a");
    map.insert(collide_b.clone(), "b");
    map.insert(normal.clone(), "c");

    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&collide_a), Some(&"a"));
    assert_eq!(map.get(&collide_b), Some(&"b"));
    assert_eq!(map.get(&normal), Some(&"c"));
}
