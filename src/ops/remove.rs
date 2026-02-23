//! Removal operation — COW path-copy delete with canonical inlining.

use std::hash::Hash;

use safe_bump::Idx;

use crate::adhash;
use crate::node::{self, Entry, Node};
use crate::store::ChampStore;

/// Outcome of a recursive remove.
pub enum RemoveOutcome<K, V> {
    /// Key was not found — tree unchanged.
    NotFound,
    /// Key was removed.
    Removed {
        /// New root of the modified subtree, or `None` if the subtree is now empty.
        node: Option<Idx<Node<K, V>>>,
        /// Wrapping `AdHash` delta to subtract from the parent's adhash.
        adhash_delta: u64,
    },
}

/// Removes `key` from the subtree rooted at `node_idx` via COW path-copy.
pub fn remove_recursive<K, V, S>(
    store: &mut S,
    node_idx: Idx<Node<K, V>>,
    hash: u64,
    key: &K,
    shift: u32,
) -> RemoveOutcome<K, V>
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
        } => remove_from_inner(
            store, data_map, node_map, data_start, children_start, adhash, hash, key, shift,
        ),
        Node::Collision {
            hash: node_hash,
            entries_start,
            entries_len,
            adhash,
        } => remove_from_collision(store, node_hash, entries_start, entries_len, adhash, hash, key),
    }
}

// ---------------------------------------------------------------------------
// Inner node remove
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn remove_from_inner<K, V, S>(
    store: &mut S,
    data_map: u32,
    node_map: u32,
    data_start: Idx<Entry<K, V>>,
    children_start: Idx<Idx<Node<K, V>>>,
    adhash: u64,
    hash: u64,
    key: &K,
    shift: u32,
) -> RemoveOutcome<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    let frag = node::fragment(hash, shift);
    let bit = node::mask(frag);
    let data_len = data_map.count_ones() as usize;
    let children_len = node_map.count_ones() as usize;

    if data_map & bit != 0 {
        let pos = node::index(data_map, bit);
        let (found, removed_contrib) = {
            let e = store.get_entry(node::offset(data_start, pos));
            let found = e.hash == hash && e.key == *key;
            let contrib = adhash::entry_adhash(e.hash, adhash::hash_one(&e.value));
            (found, contrib)
        };

        if !found {
            return RemoveOutcome::NotFound;
        }

        let new_data_map = data_map & !bit;

        // If removing the last entry and there are no children → empty subtree.
        if new_data_map == 0 && node_map == 0 {
            return RemoveOutcome::Removed { node: None, adhash_delta: removed_contrib };
        }

        let entries = build_entries_removing(store, data_start, data_len, pos);
        let new_data = alloc_or_sentinel(store.alloc_entries(entries));
        let new_node = store.alloc_node(Node::Inner {
            data_map: new_data_map,
            node_map,
            data_start: new_data,
            children_start,
            adhash: adhash.wrapping_sub(removed_contrib),
        });
        RemoveOutcome::Removed { node: Some(new_node), adhash_delta: removed_contrib }
    } else if node_map & bit != 0 {
        let child_pos = node::index(node_map, bit);
        let old_child = *store.get_child(node::offset(children_start, child_pos));
        let outcome = remove_recursive(store, old_child, hash, key, shift + node::BITS_PER_LEVEL);

        match outcome {
            RemoveOutcome::NotFound => RemoveOutcome::NotFound,
            RemoveOutcome::Removed { node: new_child, adhash_delta } => {
                if let Some(child_idx) = new_child {
                    // Child still exists — check if it should be inlined.
                    let child_node = *store.get_node(child_idx);
                    if should_inline(&child_node) {
                        inline_child(
                            store, data_map, node_map, data_start, children_start,
                            adhash, bit, child_pos, child_idx, adhash_delta,
                            data_len, children_len,
                        )
                    } else {
                        // Keep child as subtree, update pointer.
                        let children = build_children_replacing(
                            store, children_start, children_len, child_pos, child_idx,
                        );
                        let new_children = store.alloc_children(children)
                            .expect("non-empty");
                        let new_node = store.alloc_node(Node::Inner {
                            data_map,
                            node_map,
                            data_start,
                            children_start: new_children,
                            adhash: adhash.wrapping_sub(adhash_delta),
                        });
                        RemoveOutcome::Removed { node: Some(new_node), adhash_delta }
                    }
                } else {
                    // Child became empty — remove child slot.
                    let new_node_map = node_map & !bit;
                    if data_map == 0 && new_node_map == 0 {
                        return RemoveOutcome::Removed {
                            node: None,
                            adhash_delta,
                        };
                    }
                    let children = build_children_removing(
                        store, children_start, children_len, child_pos,
                    );
                    let new_children = alloc_or_sentinel(store.alloc_children(children));
                    let new_node = store.alloc_node(Node::Inner {
                        data_map,
                        node_map: new_node_map,
                        data_start,
                        children_start: new_children,
                        adhash: adhash.wrapping_sub(adhash_delta),
                    });
                    RemoveOutcome::Removed { node: Some(new_node), adhash_delta }
                }
            }
        }
    } else {
        RemoveOutcome::NotFound
    }
}

/// Canonical form: a child with exactly one entry and no children
/// should be inlined back into the parent.
const fn should_inline<K, V>(node: &Node<K, V>) -> bool {
    match node {
        Node::Inner { data_map, node_map, .. } => {
            data_map.is_power_of_two() && *node_map == 0
        }
        Node::Collision { .. } => false,
    }
}

/// Inlines a single-entry child back into the parent node.
#[allow(clippy::too_many_arguments)]
fn inline_child<K, V, S>(
    store: &mut S,
    data_map: u32,
    node_map: u32,
    data_start: Idx<Entry<K, V>>,
    children_start: Idx<Idx<Node<K, V>>>,
    adhash: u64,
    bit: u32,
    child_pos: usize,
    child_idx: Idx<Node<K, V>>,
    adhash_delta: u64,
    data_len: usize,
    children_len: usize,
) -> RemoveOutcome<K, V>
where
    K: Clone,
    V: Clone,
    S: ChampStore<K, V>,
{
    // Read the single entry from the child.
    let child = *store.get_node(child_idx);
    let child_data_start = match child {
        Node::Inner { data_start, .. } => data_start,
        Node::Collision { .. } => unreachable!("should_inline returned false for collision"),
    };
    let inlined_entry = clone_entry(store, child_data_start);

    // Remove child from children, add entry to data.
    let new_data_map = data_map | bit;
    let new_node_map = node_map & !bit;
    let data_insert_at = node::index(new_data_map, bit);

    let entries = build_entries_inserting(store, data_start, data_len, data_insert_at, inlined_entry);
    let children = build_children_removing(store, children_start, children_len, child_pos);

    let new_data = store.alloc_entries(entries).expect("non-empty after inline");
    let new_children = alloc_or_sentinel(store.alloc_children(children));

    let new_node = store.alloc_node(Node::Inner {
        data_map: new_data_map,
        node_map: new_node_map,
        data_start: new_data,
        children_start: new_children,
        adhash: adhash.wrapping_sub(adhash_delta),
    });
    RemoveOutcome::Removed { node: Some(new_node), adhash_delta }
}

// ---------------------------------------------------------------------------
// Collision node remove
// ---------------------------------------------------------------------------

fn remove_from_collision<K, V, S>(
    store: &mut S,
    node_hash: u64,
    entries_start: Idx<Entry<K, V>>,
    entries_len: u8,
    adhash: u64,
    hash: u64,
    key: &K,
) -> RemoveOutcome<K, V>
where
    K: Hash + Eq + Clone,
    V: Hash + Clone,
    S: ChampStore<K, V>,
{
    if hash != node_hash {
        return RemoveOutcome::NotFound;
    }

    let len = usize::from(entries_len);
    for i in 0..len {
        let (found, removed_contrib) = {
            let e = store.get_entry(node::offset(entries_start, i));
            let found = e.key == *key;
            let contrib = adhash::entry_adhash(e.hash, adhash::hash_one(&e.value));
            (found, contrib)
        };

        if !found {
            continue;
        }

        if len == 2 {
            // Collision with 2 entries → removing one leaves a single entry.
            // Promote it to a regular inner node at this depth.
            let other = 1 - i;
            let remaining = clone_entry(store, node::offset(entries_start, other));
            let remaining_contrib = adhash::entry_adhash(
                remaining.hash,
                adhash::hash_one(&remaining.value),
            );
            let frag = node::fragment(remaining.hash, 0);
            let bit = node::mask(frag);
            let data_start = store.alloc_entries([remaining]).expect("single entry");
            let new_node = store.alloc_node(Node::Inner {
                data_map: bit,
                node_map: 0,
                data_start,
                children_start: Idx::from_raw(0),
                adhash: remaining_contrib,
            });
            return RemoveOutcome::Removed { node: Some(new_node), adhash_delta: removed_contrib };
        }

        let entries = build_entries_removing(store, entries_start, len, i);
        let new_start = store.alloc_entries(entries).expect("at least 2 remaining");
        let new_node = store.alloc_node(Node::Collision {
            hash: node_hash,
            entries_start: new_start,
            entries_len: entries_len - 1,
            adhash: adhash.wrapping_sub(removed_contrib),
        });
        return RemoveOutcome::Removed { node: Some(new_node), adhash_delta: removed_contrib };
    }

    RemoveOutcome::NotFound
}

// ---------------------------------------------------------------------------
// Helpers (shared with insert.rs via copy — small, private)
// ---------------------------------------------------------------------------

fn clone_entry<K: Clone, V: Clone, S: ChampStore<K, V>>(
    store: &S,
    idx: Idx<Entry<K, V>>,
) -> Entry<K, V> {
    let e = store.get_entry(idx);
    Entry { hash: e.hash, key: e.key.clone(), value: e.value.clone() }
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

fn build_children_removing<K, V, S: ChampStore<K, V>>(
    store: &S,
    start: Idx<Idx<Node<K, V>>>,
    len: usize,
    at: usize,
) -> Vec<Idx<Node<K, V>>> {
    let mut out = Vec::with_capacity(len - 1);
    for i in 0..len {
        if i != at {
            out.push(*store.get_child(node::offset(start, i)));
        }
    }
    out
}

#[allow(clippy::option_if_let_else)]
const fn alloc_or_sentinel<T>(idx: Option<Idx<T>>) -> Idx<T> {
    match idx {
        Some(i) => i,
        None => Idx::from_raw(0),
    }
}
