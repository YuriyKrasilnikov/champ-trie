//! Multi-threaded CHAMP map.

use std::fmt;
use std::hash::Hash;
use std::ops;

use safe_bump::Idx;

use crate::adhash;
use crate::arena_sync::ChampArenaSync;
use crate::iter::Iter;
use crate::node::{self, Entry, Node};
use crate::ops::get::get_recursive;
use crate::ops::insert::insert_recursive;
use crate::ops::remove::{RemoveOutcome, remove_recursive};
use crate::store::ChampStore;
use crate::{ChampCheckpoint, InsertResult};

/// Persistent hash map based on a CHAMP trie, multi-threaded.
///
/// Identical API to [`ChampMap`](crate::ChampMap) but backed by
/// [`SharedArena`](safe_bump::SharedArena) for `Send + Sync` support.
pub struct ChampMapSync<K, V> {
    store: ChampArenaSync<K, V>,
    root: Option<safe_bump::Idx<crate::node::Node<K, V>>>,
    size: usize,
    adhash: u64,
}

// ---------------------------------------------------------------------------
// Construction & accessors — no trait bounds
// ---------------------------------------------------------------------------

impl<K, V> ChampMapSync<K, V> {
    /// Creates an empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            store: ChampArenaSync::new(),
            root: None,
            size: 0,
            adhash: 0,
        }
    }

    /// Returns the number of key-value pairs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.size
    }

    /// Returns `true` if the map contains no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the current `AdHash` value.
    #[must_use]
    pub const fn adhash(&self) -> u64 {
        self.adhash
    }

    /// Saves the current map state for later rollback.
    #[must_use]
    pub fn checkpoint(&self) -> ChampCheckpoint<K, V> {
        ChampCheckpoint {
            store: self.store.checkpoint(),
            root: self.root,
            size: self.size,
            adhash: self.adhash,
        }
    }

    /// Returns the total number of allocated items in each arena:
    /// `(nodes, entries, children)`.
    ///
    /// Includes dead COW copies — reflects true memory footprint.
    #[must_use]
    pub fn arena_len(&self) -> (usize, usize, usize) {
        self.store.arena_len()
    }

    /// Restores the map to a previously saved checkpoint.
    pub fn rollback(&mut self, cp: ChampCheckpoint<K, V>) {
        self.store.rollback(cp.store);
        self.root = cp.root;
        self.size = cp.size;
        self.adhash = cp.adhash;
    }
}

// ---------------------------------------------------------------------------
// Read operations
// ---------------------------------------------------------------------------

impl<K: Hash + Eq, V> ChampMapSync<K, V> {
    /// Returns a reference to the value associated with `key`.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        let root = self.root?;
        get_recursive(&self.store, root, adhash::hash_one(key), key, 0)
    }

    /// Returns `true` if the map contains the given key.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
}

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

impl<K: Hash + Eq + Clone, V: Hash + Clone> ChampMapSync<K, V> {
    /// Inserts a key-value pair into the map.
    ///
    /// Returns [`Inserted`](InsertResult::Inserted) if the key was new, or
    /// [`Updated`](InsertResult::Updated) if an existing value was replaced.
    ///
    /// # Panics
    ///
    /// Panics if internal arena allocation returns an unexpected `None`.
    pub fn insert(&mut self, key: K, value: V) -> InsertResult {
        let hash = adhash::hash_one(&key);
        let entry = Entry { hash, key, value };

        if let Some(root) = self.root {
            let outcome = insert_recursive(&mut self.store, root, entry, 0);
            self.root = Some(outcome.node);
            self.adhash = self.adhash.wrapping_add(outcome.adhash_delta);
            if outcome.inserted {
                self.size += 1;
                InsertResult::Inserted
            } else {
                InsertResult::Updated
            }
        } else {
            let value_hash = adhash::hash_one(&entry.value);
            let contribution = adhash::entry_adhash(hash, value_hash);
            let frag = node::fragment(hash, 0);
            let bit = node::mask(frag);
            let data_start = self
                .store
                .alloc_entries(std::iter::once(entry))
                .expect("single entry");
            let new_node = self.store.alloc_node(Node::Inner {
                data_map: bit,
                node_map: 0,
                data_start,
                children_start: Idx::from_raw(0),
                adhash: contribution,
            });
            self.root = Some(new_node);
            self.size = 1;
            self.adhash = contribution;
            InsertResult::Inserted
        }
    }

    /// Removes a key from the map. Returns `true` if the key was present.
    pub fn remove(&mut self, key: &K) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        let hash = adhash::hash_one(key);
        match remove_recursive(&mut self.store, root, hash, key, 0) {
            RemoveOutcome::NotFound => false,
            RemoveOutcome::Removed { node, adhash_delta } => {
                self.root = node;
                self.size -= 1;
                self.adhash = self.adhash.wrapping_sub(adhash_delta);
                true
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Iterator stubs
// ---------------------------------------------------------------------------

impl<K, V> ChampMapSync<K, V> {
    /// Returns an iterator over `(&K, &V)` pairs.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(&self.store, self.root)
    }
}

// ---------------------------------------------------------------------------
// Trait impls
// ---------------------------------------------------------------------------

impl<K, V> Default for ChampMapSync<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> fmt::Debug for ChampMapSync<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChampMapSync")
            .field("len", &self.size)
            .field("adhash", &format_args!("{:#018x}", self.adhash))
            .finish_non_exhaustive()
    }
}

impl<K: Hash + Eq + Clone, V: Hash + Clone> Extend<(K, V)> for ChampMapSync<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K: Hash + Eq + Clone, V: Hash + Clone> FromIterator<(K, V)> for ChampMapSync<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<K: Hash + Eq, V> ops::Index<&K> for ChampMapSync<K, V> {
    type Output = V;

    fn index(&self, key: &K) -> &V {
        self.get(key).expect("key not found")
    }
}

impl<'a, K, V> IntoIterator for &'a ChampMapSync<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}
