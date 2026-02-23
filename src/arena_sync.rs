//! Shared-arena-backed storage (multi-threaded).

use safe_bump::{Idx, SharedArena};

use crate::node::{Entry, Node};
use crate::store::{ChampStore, StoreCheckpoint};

/// Thread-safe storage backend using three [`SharedArena`]s.
pub struct ChampArenaSync<K, V> {
    nodes: SharedArena<Node<K, V>>,
    entries: SharedArena<Entry<K, V>>,
    children: SharedArena<Idx<Node<K, V>>>,
}

impl<K, V> ChampArenaSync<K, V> {
    /// Creates an empty store.
    pub const fn new() -> Self {
        Self {
            nodes: SharedArena::new(),
            entries: SharedArena::new(),
            children: SharedArena::new(),
        }
    }
}

impl<K, V> Default for ChampArenaSync<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> ChampStore<K, V> for ChampArenaSync<K, V> {
    fn alloc_node(&mut self, node: Node<K, V>) -> Idx<Node<K, V>> {
        self.nodes.alloc(node)
    }

    fn get_node(&self, idx: Idx<Node<K, V>>) -> &Node<K, V> {
        self.nodes.get(idx)
    }

    fn alloc_entries(
        &mut self,
        iter: impl IntoIterator<Item = Entry<K, V>>,
    ) -> Option<Idx<Entry<K, V>>> {
        self.entries.alloc_extend(iter)
    }

    fn get_entry(&self, idx: Idx<Entry<K, V>>) -> &Entry<K, V> {
        self.entries.get(idx)
    }

    fn alloc_children(
        &mut self,
        iter: impl IntoIterator<Item = Idx<Node<K, V>>>,
    ) -> Option<Idx<Idx<Node<K, V>>>> {
        self.children.alloc_extend(iter)
    }

    fn get_child(&self, idx: Idx<Idx<Node<K, V>>>) -> &Idx<Node<K, V>> {
        self.children.get(idx)
    }

    fn checkpoint(&self) -> StoreCheckpoint<K, V> {
        StoreCheckpoint {
            nodes: self.nodes.checkpoint(),
            entries: self.entries.checkpoint(),
            children: self.children.checkpoint(),
        }
    }

    fn rollback(&mut self, cp: StoreCheckpoint<K, V>) {
        self.nodes.rollback(cp.nodes);
        self.entries.rollback(cp.entries);
        self.children.rollback(cp.children);
    }

    fn arena_len(&self) -> (usize, usize, usize) {
        (self.nodes.len(), self.entries.len(), self.children.len())
    }
}
