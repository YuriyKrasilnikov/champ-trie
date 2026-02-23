use crate::ChampMap;

/// Insertion order must not affect the resulting structure.
/// Same set of entries → same adhash.
#[test]
fn insert_order_abc_cba_bca() {
    let orders: [&[(i32, i32)]; 3] = [
        &[(1, 10), (2, 20), (3, 30)],
        &[(3, 30), (2, 20), (1, 10)],
        &[(2, 20), (3, 30), (1, 10)],
    ];

    let maps: Vec<ChampMap<i32, i32>> = orders
        .iter()
        .map(|pairs| {
            let mut m = ChampMap::new();
            for &(k, v) in *pairs {
                m.insert(k, v);
            }
            m
        })
        .collect();

    // All adhashes must be equal.
    assert_eq!(maps[0].adhash(), maps[1].adhash());
    assert_eq!(maps[1].adhash(), maps[2].adhash());
    assert_eq!(maps[0].len(), maps[1].len());
}

/// Larger set — 100 entries, three orderings.
#[test]
fn insert_order_100_entries() {
    let entries: Vec<(u64, u64)> = (0..100).map(|i| (i, i * 7)).collect();

    let mut forward = ChampMap::new();
    for &(k, v) in &entries {
        forward.insert(k, v);
    }

    let mut backward = ChampMap::new();
    for &(k, v) in entries.iter().rev() {
        backward.insert(k, v);
    }

    let mut interleaved = ChampMap::new();
    for &(k, v) in entries.iter().step_by(2) {
        interleaved.insert(k, v);
    }
    for &(k, v) in entries.iter().skip(1).step_by(2) {
        interleaved.insert(k, v);
    }

    assert_eq!(forward.adhash(), backward.adhash());
    assert_eq!(forward.adhash(), interleaved.adhash());
    assert_eq!(forward.len(), 100);
}

/// After overwrite, order independence still holds.
#[test]
fn overwrite_preserves_canonicity() {
    let mut map_a = ChampMap::new();
    map_a.insert(1, 10);
    map_a.insert(2, 20);
    map_a.insert(1, 11); // overwrite

    let mut map_b = ChampMap::new();
    map_b.insert(2, 20);
    map_b.insert(1, 11); // insert final value directly

    assert_eq!(map_a.adhash(), map_b.adhash());
    assert_eq!(map_a.len(), map_b.len());
}

/// After delete, order independence holds.
#[test]
fn delete_preserves_canonicity() {
    let mut map_a = ChampMap::new();
    map_a.insert(1, 10);
    map_a.insert(2, 20);
    map_a.insert(3, 30);
    map_a.remove(&2);

    let mut map_b = ChampMap::new();
    map_b.insert(3, 30);
    map_b.insert(1, 10);

    assert_eq!(map_a.adhash(), map_b.adhash());
    assert_eq!(map_a.len(), map_b.len());
}
