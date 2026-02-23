use crate::ChampMap;
use crate::adhash::{entry_adhash, hash_one};

/// φ(∅) = 0.
#[test]
fn empty_adhash_is_zero() {
    let map: ChampMap<String, String> = ChampMap::new();
    assert_eq!(map.adhash(), 0);
}

/// φ(S ∪ {e}) = φ(S) + f(e).
#[test]
fn incremental_insert() {
    let mut map = ChampMap::new();
    let h0 = map.adhash();

    map.insert("a", 1);
    let h1 = map.adhash();
    let expected = h0.wrapping_add(entry_adhash(hash_one(&"a"), hash_one(&1)));
    assert_eq!(h1, expected);

    map.insert("b", 2);
    let h2 = map.adhash();
    let expected2 = h1.wrapping_add(entry_adhash(hash_one(&"b"), hash_one(&2)));
    assert_eq!(h2, expected2);
}

/// Insert + remove roundtrip: φ returns to 0.
#[test]
fn roundtrip_to_zero() {
    let mut map = ChampMap::new();
    map.insert(1, 100);
    map.insert(2, 200);
    map.insert(3, 300);
    map.remove(&1);
    map.remove(&2);
    map.remove(&3);
    assert_eq!(map.adhash(), 0);
}

/// Commutativity: φ({a,b}) = φ({b,a}).
#[test]
fn commutativity() {
    let mut m1 = ChampMap::new();
    m1.insert("x", 10);
    m1.insert("y", 20);

    let mut m2 = ChampMap::new();
    m2.insert("y", 20);
    m2.insert("x", 10);

    assert_eq!(m1.adhash(), m2.adhash());
}

/// Two seeds prevent degeneration: even when `hash(value) = 0`, adhash
/// is still non-trivial.
#[test]
fn two_seed_no_degeneration() {
    // entry_adhash(key_hash, 0) should still be non-zero for nonzero key_hash.
    let key_hash = hash_one(&42_u64);
    let contribution = entry_adhash(key_hash, 0);
    assert_ne!(contribution, 0);
}

/// Mixing function is not symmetric: f(k, v) ≠ f(v, k) in general.
#[test]
fn mixing_not_symmetric() {
    let a = entry_adhash(hash_one(&1_i32), hash_one(&2_i32));
    let b = entry_adhash(hash_one(&2_i32), hash_one(&1_i32));
    // Very unlikely to be equal with different seeds.
    assert_ne!(a, b);
}

/// Overwrite changes adhash: φ(S with v1) ≠ φ(S with v2).
#[test]
fn overwrite_changes_adhash() {
    let mut map = ChampMap::new();
    map.insert("key", 1);
    let h1 = map.adhash();
    map.insert("key", 2);
    let h2 = map.adhash();
    assert_ne!(h1, h2);
}
