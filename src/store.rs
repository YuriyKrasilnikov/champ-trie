//! Storage abstraction for CHAMP trie operations.

use safe_bump::{Checkpoint, Idx};

use crate::node::{Entry, Node};

/// Saved state of the three storage arenas.
pub struct StoreCheckpoint<K, V> {
    /// Nodes arena checkpoint.
    pub nodes: Checkpoint<Node<K, V>>,
    /// Entries arena checkpoint.
    pub entries: Checkpoint<Entry<K, V>>,
    /// Children arena checkpoint.
    pub children: Checkpoint<Idx<Node<K, V>>>,
}

// StoreCheckpoint contains only Checkpoint<T> values (Copy) — no K/V data.

impl<K, V> Clone for StoreCheckpoint<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for StoreCheckpoint<K, V> {}

/// Storage backend for CHAMP operations.
///
/// Abstracts over [`Arena`](safe_bump::Arena) (single-thread) and
/// [`SharedArena`](safe_bump::SharedArena) (multi-thread) backends.
pub trait ChampStore<K, V> {
    /// Allocates a single node, returning its index.
    fn alloc_node(&mut self, node: Node<K, V>) -> Idx<Node<K, V>>;

    /// Returns a reference to the node at `idx`.
    fn get_node(&self, idx: Idx<Node<K, V>>) -> &Node<K, V>;

    /// Allocates a contiguous block of entries, returning the index of the
    /// first one. Returns `None` if the iterator is empty.
    fn alloc_entries(
        &mut self,
        iter: impl IntoIterator<Item = Entry<K, V>>,
    ) -> Option<Idx<Entry<K, V>>>;

    /// Returns a reference to the entry at `idx`.
    fn get_entry(&self, idx: Idx<Entry<K, V>>) -> &Entry<K, V>;

    /// Allocates a contiguous block of child node indices, returning the
    /// index of the first one. Returns `None` if the iterator is empty.
    fn alloc_children(
        &mut self,
        iter: impl IntoIterator<Item = Idx<Node<K, V>>>,
    ) -> Option<Idx<Idx<Node<K, V>>>>;

    /// Returns a reference to the child index at `idx`.
    fn get_child(&self, idx: Idx<Idx<Node<K, V>>>) -> &Idx<Node<K, V>>;

    /// Saves the current state of all three arenas.
    fn checkpoint(&self) -> StoreCheckpoint<K, V>;

    /// Rolls back all three arenas to a previous checkpoint.
    fn rollback(&mut self, cp: StoreCheckpoint<K, V>);

    /// Returns the total number of allocated items in each arena:
    /// `(nodes, entries, children)`.
    ///
    /// Includes dead COW copies — reflects true memory footprint.
    fn arena_len(&self) -> (usize, usize, usize);
}
