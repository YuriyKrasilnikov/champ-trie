//! Iterator types for CHAMP maps.

use safe_bump::Idx;

use crate::node::{self, Entry, Node};
use crate::store::ChampStore;

/// Iterator over references to key-value pairs in a [`ChampMap`](crate::ChampMap).
pub struct Iter<'a, K, V> {
    entries: Vec<(&'a K, &'a V)>,
    pos: usize,
}

impl<'a, K, V> Iter<'a, K, V> {
    /// Creates an iterator by collecting all live entries via DFS.
    pub fn new<S: ChampStore<K, V>>(store: &'a S, root: Option<Idx<Node<K, V>>>) -> Self {
        let mut entries = Vec::new();
        if let Some(idx) = root {
            collect(store, idx, &mut entries);
        }
        Self { entries, pos: 0 }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.entries.len() {
            let item = self.entries[self.pos];
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.entries.len() - self.pos;
        (remaining, Some(remaining))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

/// DFS collect all `(&K, &V)` from the subtree rooted at `node_idx`.
fn collect<'a, K, V, S: ChampStore<K, V>>(
    store: &'a S,
    node_idx: Idx<Node<K, V>>,
    out: &mut Vec<(&'a K, &'a V)>,
) {
    match *store.get_node(node_idx) {
        Node::Inner {
            data_map,
            node_map,
            data_start,
            children_start,
            ..
        } => {
            let data_len = data_map.count_ones() as usize;
            let children_len = node_map.count_ones() as usize;

            for i in 0..data_len {
                let e: &'a Entry<K, V> = store.get_entry(node::offset(data_start, i));
                out.push((&e.key, &e.value));
            }

            for i in 0..children_len {
                let child = *store.get_child(node::offset(children_start, i));
                collect(store, child, out);
            }
        }
        Node::Collision {
            entries_start,
            entries_len,
            ..
        } => {
            for i in 0..usize::from(entries_len) {
                let e: &'a Entry<K, V> = store.get_entry(node::offset(entries_start, i));
                out.push((&e.key, &e.value));
            }
        }
    }
}
