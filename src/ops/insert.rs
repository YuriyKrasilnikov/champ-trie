//! Insertion operation — COW path-copy insert with `AdHash` maintenance.

use std::hash::Hash;

use safe_bump::Idx;

use crate::adhash;
use crate::node::{self, Entry, Node};
use crate::store::ChampStore;

/// Outcome of a recursive insert.
pub struct InsertOutcome<K, V> {
    /// Index of the new (COW-copied) root of the modified subtree.
    pub node: Idx<Node<K, V>>,
    /// Wrapping `AdHash` delta to add to the parent's adhash.
    pub adhash_delta: u64,
    /// `true` if a new key was inserted, `false` if an existing value was updated.
    pub inserted: bool,
}

/// Inserts `entry` into the subtree rooted at `node_idx` via COW path-copy.
pub fn insert_recursive<K, V, S>(
    store: &mut S,
    node_idx: Idx<Node<K, V>>,
    entry: Entry<K, V>,
    shift: u32,
) -> InsertOutcome<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    let node = *store.get_node(node_idx);
    match node {
        Node::Inner {
            data_map,
            node_map,
            data_start,
            children_start,
            adhash,
        } => insert_into_inner(
            store,
            data_map,
            node_map,
            data_start,
            children_start,
            adhash,
            entry,
            shift,
        ),
        Node::Collision {
            hash: node_hash,
            entries_start,
            entries_len,
            adhash,
        } => insert_into_collision(store, node_hash, entries_start, entries_len, adhash, entry),
    }
}

// ---------------------------------------------------------------------------
// Inner node insert
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn insert_into_inner<K, V, S>(
    store: &mut S,
    data_map: u32,
    node_map: u32,
    data_start: Idx<Entry<K, V>>,
    children_start: Idx<Idx<Node<K, V>>>,
    adhash: u64,
    entry: Entry<K, V>,
    shift: u32,
) -> InsertOutcome<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    let frag = node::fragment(entry.hash, shift);
    let bit = node::mask(frag);
    let data_len = data_map.count_ones() as usize;
    let children_len = node_map.count_ones() as usize;

    if data_map & bit != 0 {
        let pos = node::index(data_map, bit);
        let (existing_hash, existing_key_eq, old_contrib) = {
            let e = store.get_entry(node::offset(data_start, pos));
            let eq = e.hash == entry.hash && e.key == entry.key;
            let contrib = adhash::entry_adhash(e.hash, adhash::hash_one(&e.value));
            (e.hash, eq, contrib)
        };

        if existing_key_eq {
            // Same key → update value.
            let new_contrib = adhash::entry_adhash(entry.hash, adhash::hash_one(&entry.value));
            let delta = new_contrib.wrapping_sub(old_contrib);
            let entries = build_entries_replacing(store, data_start, data_len, pos, entry);
            let new_data = store.alloc_entries(entries).expect("non-empty");
            let new_node = store.alloc_node(Node::Inner {
                data_map,
                node_map,
                data_start: new_data,
                children_start,
                adhash: adhash.wrapping_add(delta),
            });
            InsertOutcome {
                node: new_node,
                adhash_delta: delta,
                inserted: false,
            }
        } else {
            // Different key at same position → push both into a subtree.
            let existing_cloned = clone_entry(store, node::offset(data_start, pos));
            let new_contrib = adhash::entry_adhash(entry.hash, adhash::hash_one(&entry.value));
            let _ = existing_hash; // used above for eq check

            let subtree =
                create_subtree(store, existing_cloned, entry, shift + node::BITS_PER_LEVEL);

            let new_data_map = data_map & !bit;
            let new_node_map = node_map | bit;
            let child_pos = node::index(new_node_map, bit);

            let entries = build_entries_removing(store, data_start, data_len, pos);
            let children =
                build_children_inserting(store, children_start, children_len, child_pos, subtree);

            let new_data = alloc_or_sentinel(store.alloc_entries(entries));
            let new_children = store.alloc_children(children).expect("non-empty");

            let new_node = store.alloc_node(Node::Inner {
                data_map: new_data_map,
                node_map: new_node_map,
                data_start: new_data,
                children_start: new_children,
                adhash: adhash.wrapping_add(new_contrib),
            });
            InsertOutcome {
                node: new_node,
                adhash_delta: new_contrib,
                inserted: true,
            }
        }
    } else if node_map & bit != 0 {
        // Position has child subtree → recurse.
        let child_pos = node::index(node_map, bit);
        let old_child = *store.get_child(node::offset(children_start, child_pos));
        let outcome = insert_recursive(store, old_child, entry, shift + node::BITS_PER_LEVEL);

        let children =
            build_children_replacing(store, children_start, children_len, child_pos, outcome.node);
        let new_children = store.alloc_children(children).expect("non-empty");

        let new_node = store.alloc_node(Node::Inner {
            data_map,
            node_map,
            data_start,
            children_start: new_children,
            adhash: adhash.wrapping_add(outcome.adhash_delta),
        });
        InsertOutcome {
            node: new_node,
            adhash_delta: outcome.adhash_delta,
            inserted: outcome.inserted,
        }
    } else {
        // Position empty → add inline entry.
        let new_data_map = data_map | bit;
        let insert_at = node::index(new_data_map, bit);
        let new_contrib = adhash::entry_adhash(entry.hash, adhash::hash_one(&entry.value));
        let entries = build_entries_inserting(store, data_start, data_len, insert_at, entry);
        let new_data = store.alloc_entries(entries).expect("non-empty");

        let new_node = store.alloc_node(Node::Inner {
            data_map: new_data_map,
            node_map,
            data_start: new_data,
            children_start,
            adhash: adhash.wrapping_add(new_contrib),
        });
        InsertOutcome {
            node: new_node,
            adhash_delta: new_contrib,
            inserted: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Collision node insert
// ---------------------------------------------------------------------------

fn insert_into_collision<K, V, S>(
    store: &mut S,
    node_hash: u64,
    entries_start: Idx<Entry<K, V>>,
    entries_len: u8,
    adhash: u64,
    entry: Entry<K, V>,
) -> InsertOutcome<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    let len = usize::from(entries_len);

    // Search for existing key.
    for i in 0..len {
        let (key_eq, old_contrib) = {
            let e = store.get_entry(node::offset(entries_start, i));
            let eq = e.key == entry.key;
            let contrib = adhash::entry_adhash(e.hash, adhash::hash_one(&e.value));
            (eq, contrib)
        };
        if key_eq {
            let new_contrib = adhash::entry_adhash(entry.hash, adhash::hash_one(&entry.value));
            let delta = new_contrib.wrapping_sub(old_contrib);
            let entries = build_entries_replacing(store, entries_start, len, i, entry);
            let new_start = store.alloc_entries(entries).expect("non-empty");
            let new_node = store.alloc_node(Node::Collision {
                hash: node_hash,
                entries_start: new_start,
                entries_len,
                adhash: adhash.wrapping_add(delta),
            });
            return InsertOutcome {
                node: new_node,
                adhash_delta: delta,
                inserted: false,
            };
        }
    }

    // Key not found → append.
    let new_contrib = adhash::entry_adhash(entry.hash, adhash::hash_one(&entry.value));
    let new_len = entries_len
        .checked_add(1)
        .expect("collision node overflow (>255 entries)");
    let mut entries = Vec::with_capacity(len + 1);
    for i in 0..len {
        entries.push(clone_entry(store, node::offset(entries_start, i)));
    }
    entries.push(entry);
    let new_start = store.alloc_entries(entries).expect("non-empty");
    let new_node = store.alloc_node(Node::Collision {
        hash: node_hash,
        entries_start: new_start,
        entries_len: new_len,
        adhash: adhash.wrapping_add(new_contrib),
    });
    InsertOutcome {
        node: new_node,
        adhash_delta: new_contrib,
        inserted: true,
    }
}

// ---------------------------------------------------------------------------
// Batch subtree creation
// ---------------------------------------------------------------------------

/// Creates a subtree from two entries that collide at the current depth.
///
/// Recursively descends until hash fragments differ, or creates a collision
/// node at `MAX_SHIFT`.
fn create_subtree<K, V, S>(
    store: &mut S,
    e1: Entry<K, V>,
    e2: Entry<K, V>,
    shift: u32,
) -> Idx<Node<K, V>>
where
    K: Hash + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    if shift > node::MAX_SHIFT {
        let hash = e1.hash;
        let c1 = adhash::entry_adhash(e1.hash, adhash::hash_one(&e1.value));
        let c2 = adhash::entry_adhash(e2.hash, adhash::hash_one(&e2.value));
        let start = store.alloc_entries([e1, e2]).expect("two entries");
        return store.alloc_node(Node::Collision {
            hash,
            entries_start: start,
            entries_len: 2,
            adhash: c1.wrapping_add(c2),
        });
    }

    let f1 = node::fragment(e1.hash, shift);
    let f2 = node::fragment(e2.hash, shift);

    if f1 == f2 {
        let child = create_subtree(store, e1, e2, shift + node::BITS_PER_LEVEL);
        let child_adhash = store.get_node(child).adhash();
        let children_start = store.alloc_children([child]).expect("one child");
        store.alloc_node(Node::Inner {
            data_map: 0,
            node_map: node::mask(f1),
            data_start: Idx::from_raw(0),
            children_start,
            adhash: child_adhash,
        })
    } else {
        let c1 = adhash::entry_adhash(e1.hash, adhash::hash_one(&e1.value));
        let c2 = adhash::entry_adhash(e2.hash, adhash::hash_one(&e2.value));
        let entries: [Entry<K, V>; 2] = if f1 < f2 { [e1, e2] } else { [e2, e1] };
        let data_start = store.alloc_entries(entries).expect("two entries");
        store.alloc_node(Node::Inner {
            data_map: node::mask(f1) | node::mask(f2),
            node_map: 0,
            data_start,
            children_start: Idx::from_raw(0),
            adhash: c1.wrapping_add(c2),
        })
    }
}

// ---------------------------------------------------------------------------
// Entry / children block builders
// ---------------------------------------------------------------------------

fn clone_entry<K: Clone, V: Clone, S: ChampStore<K, V>>(
    store: &S,
    idx: Idx<Entry<K, V>>,
) -> Entry<K, V> {
    let e = store.get_entry(idx);
    Entry {
        hash: e.hash,
        key: e.key.clone(),
        value: e.value.clone(),
    }
}

fn build_entries_inserting<K: Clone, V: Clone, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Entry<K, V>>,
    len: usize,
    at: usize,
    entry: Entry<K, V>,
) -> Vec<Entry<K, V>> {
    let mut out = Vec::with_capacity(len + 1);
    for i in 0..at {
        out.push(clone_entry(store, node::offset(start, i)));
    }
    out.push(entry);
    for i in at..len {
        out.push(clone_entry(store, node::offset(start, i)));
    }
    out
}

fn build_entries_replacing<K: Clone, V: Clone, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Entry<K, V>>,
    len: usize,
    at: usize,
    entry: Entry<K, V>,
) -> Vec<Entry<K, V>> {
    let mut out = Vec::with_capacity(len);
    for i in 0..at {
        out.push(clone_entry(store, node::offset(start, i)));
    }
    out.push(entry);
    for i in (at + 1)..len {
        out.push(clone_entry(store, node::offset(start, i)));
    }
    out
}

fn build_entries_removing<K: Clone, V: Clone, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Entry<K, V>>,
    len: usize,
    at: usize,
) -> Vec<Entry<K, V>> {
    let mut out = Vec::with_capacity(len - 1);
    for i in 0..len {
        if i != at {
            out.push(clone_entry(store, node::offset(start, i)));
        }
    }
    out
}

fn build_children_inserting<K, V, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Idx<Node<K, V>>>,
    len: usize,
    at: usize,
    child: Idx<Node<K, V>>,
) -> Vec<Idx<Node<K, V>>> {
    let mut out = Vec::with_capacity(len + 1);
    for i in 0..at {
        out.push(*store.get_child(node::offset(start, i)));
    }
    out.push(child);
    for i in at..len {
        out.push(*store.get_child(node::offset(start, i)));
    }
    out
}

fn build_children_replacing<K, V, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Idx<Node<K, V>>>,
    len: usize,
    at: usize,
    child: Idx<Node<K, V>>,
) -> Vec<Idx<Node<K, V>>> {
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if i == at {
            out.push(child);
        } else {
            out.push(*store.get_child(node::offset(start, i)));
        }
    }
    out
}

/// Returns the index from an `Option`, using a sentinel for `None`.
///
/// Used when a bitmap is zero (no entries/children) and the start index
/// is dead state — never accessed because the bitmap guards it.
#[allow(clippy::option_if_let_else)]
const fn alloc_or_sentinel<T>(idx: Option<Idx<T>>) -> Idx<T> {
    match idx {
        Some(i) => i,
        None => Idx::from_raw(0),
    }
}
