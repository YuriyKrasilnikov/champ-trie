use crate::ChampMap;

/// 1000 entries: insert all, verify all, remove all.
#[test]
fn thousand_entries() {
    let mut map = ChampMap::new();
    for i in 0_u64..1000 {
        map.insert(i, i * 3);
    }
    assert_eq!(map.len(), 1000);

    for i in 0_u64..1000 {
        assert_eq!(map.get(&i), Some(&(i * 3)), "missing key {i}");
    }

    for i in 0_u64..1000 {
        assert!(map.remove(&i).is_some(), "failed to remove key {i}");
    }
    assert!(map.is_empty());
    assert_eq!(map.adhash(), 0);
}

/// Deep trie: keys with shared hash prefixes force deeper nodes.
#[test]
fn deep_shared_prefixes() {
    let mut map = ChampMap::new();
    // Sequential integers often share hash prefix bits,
    // forcing deeper trie nodes.
    for i in 0_u64..500 {
        map.insert(i, i);
    }
    assert_eq!(map.len(), 500);
    for i in 0_u64..500 {
        assert_eq!(map.get(&i), Some(&i));
    }
}

/// Insert + overwrite + remove interleaved.
#[test]
fn interleaved_operations() {
    let mut map = ChampMap::new();
    for i in 0_u64..200 {
        map.insert(i, i);
    }
    // Overwrite even keys.
    for i in (0_u64..200).step_by(2) {
        map.insert(i, i + 1000);
    }
    // Remove odd keys.
    for i in (1_u64..200).step_by(2) {
        assert!(map.remove(&i).is_some());
    }
    assert_eq!(map.len(), 100);
    for i in (0_u64..200).step_by(2) {
        assert_eq!(map.get(&i), Some(&(i + 1000)));
    }
}
