//! Persistent hash map based on CHAMP.
//!
//! CHAMP (Compressed Hash-Array Mapped Prefix-tree) is a refined HAMT that
//! guarantees **canonical form**: the same set of key-value pairs always
//! produces the same trie structure, regardless of insertion order.
//!
//! # Key properties
//!
//! - **Canonical form**: same contents = same structure
//! - **O(1) structural equality**: via incrementally maintained `AdHash`
//! - **COW structural sharing**: cheap copy, mutate-on-write
//! - **Zero `unsafe`**: enforced by `#![forbid(unsafe_code)]`
//!
//! # References
//!
//! - Steindorfer & Vinju, 2015 — "Optimizing Hash-Array Mapped Tries
//!   for Fast and Lean Immutable JVM Collections", OOPSLA 2015
//! - Bagwell, 2001 — "Ideal Hash Trees"

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::fmt;

use safe_bump::Idx;

pub mod adhash;
pub mod iter;
pub mod node;
pub mod store;

mod arena;
mod arena_sync;
mod map;
mod map_sync;
mod ops;

#[cfg(test)]
mod tests;

pub use map::ChampMap;
pub use map_sync::ChampMapSync;

/// Saved map state for rollback.
///
/// Created by [`ChampMap::checkpoint`] or [`ChampMapSync::checkpoint`].
/// Restoring via `rollback` discards all changes made after the checkpoint.
pub struct ChampCheckpoint<K, V> {
    /// Three-arena store checkpoint.
    pub store: store::StoreCheckpoint<K, V>,
    /// Root node index at checkpoint time.
    pub root: Option<Idx<node::Node<K, V>>>,
    /// Entry count at checkpoint time.
    pub size: usize,
    /// `AdHash` at checkpoint time.
    pub adhash: u64,
}

// ChampCheckpoint contains only indices and primitives — no actual K/V data.

impl<K, V> Clone for ChampCheckpoint<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for ChampCheckpoint<K, V> {}

impl<K, V> fmt::Debug for ChampCheckpoint<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChampCheckpoint")
            .field("size", &self.size)
            .field("adhash", &self.adhash)
            .finish_non_exhaustive()
    }
}
