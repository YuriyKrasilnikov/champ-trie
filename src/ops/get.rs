//! Lookup operation — traverses the trie to find a key.

use safe_bump::Idx;

use crate::node::{self, Node};
use crate::store::ChampStore;

/// Searches for `key` in the subtree rooted at `node_idx`.
///
/// Returns a reference to the value if found.
pub fn get_recursive<'a, K, V, S>(
    store: &'a S,
    node_idx: Idx<Node<K, V>>,
    hash: u64,
    key: &K,
    shift: u32,
) -> Option<&'a V>
where
    K: Eq + 'a,
    V: 'a,
    S: ChampStore<K, V>,
{
    match *store.get_node(node_idx) {
        Node::Inner {
            data_map,
            node_map,
            data_start,
            children_start,
            ..
        } => {
            let frag = node::fragment(hash, shift);
            let bit = node::mask(frag);

            if data_map & bit != 0 {
                // Position has an inline entry.
                let idx = node::index(data_map, bit);
                let entry = store.get_entry(node::offset(data_start, idx));
                if entry.hash == hash && entry.key == *key {
                    Some(&entry.value)
                } else {
                    None
                }
            } else if node_map & bit != 0 {
                // Position has a child subtree — recurse.
                let idx = node::index(node_map, bit);
                let child_idx = *store.get_child(node::offset(children_start, idx));
                get_recursive(store, child_idx, hash, key, shift + node::BITS_PER_LEVEL)
            } else {
                // Position is empty.
                None
            }
        }
        Node::Collision {
            hash: node_hash,
            entries_start,
            entries_len,
            ..
        } => {
            if hash != node_hash {
                return None;
            }
            // Linear search through collision entries.
            for i in 0..usize::from(entries_len) {
                let entry = store.get_entry(node::offset(entries_start, i));
                if entry.key == *key {
                    return Some(&entry.value);
                }
            }
            None
        }
    }
}
