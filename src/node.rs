//! CHAMP trie node types and bitmap helpers.

use std::fmt;

use safe_bump::Idx;

/// Bits per trie level (5 → 32-way branching).
pub const BITS_PER_LEVEL: u32 = 5;

/// Maximum bit-shift value (depth 12, last level uses 4 bits).
pub const MAX_SHIFT: u32 = 60;

/// Inline entry storing a key-value pair with its precomputed hash.
pub struct Entry<K, V> {
    /// Precomputed 64-bit hash of the key.
    pub hash: u64,
    /// The key.
    pub key: K,
    /// The value.
    pub value: V,
}

/// CHAMP trie node.
///
/// Two variants maintain the canonical form invariant:
/// - [`Inner`](Self::Inner) — bitmap-compressed node at depth `d < D`
/// - [`Collision`](Self::Collision) — linear node for full 64-bit hash collisions
pub enum Node<K, V> {
    /// Bitmap-compressed inner node.
    ///
    /// Invariant: `data_map & node_map == 0` (disjoint positions).
    Inner {
        /// Bitmap of positions occupied by inline entries.
        data_map: u32,
        /// Bitmap of positions occupied by child subtrees.
        node_map: u32,
        /// Index of the first inline entry in the entries arena.
        data_start: Idx<Entry<K, V>>,
        /// Index of the first child pointer in the children arena.
        children_start: Idx<Idx<Self>>,
        /// `AdHash` of this subtree.
        adhash: u64,
    },
    /// Collision node for keys sharing the same 64-bit hash.
    ///
    /// Invariant: `entries_len >= 2`.
    Collision {
        /// The shared 64-bit hash value.
        hash: u64,
        /// Index of the first entry in the entries arena.
        entries_start: Idx<Entry<K, V>>,
        /// Number of collision entries.
        entries_len: u8,
        /// `AdHash` of this subtree.
        adhash: u64,
    },
}

// ---------------------------------------------------------------------------
// Bitmap helpers
// ---------------------------------------------------------------------------

/// Extracts the 5-bit hash fragment at the given bit-shift depth.
#[inline]
#[must_use]
pub const fn fragment(hash: u64, shift: u32) -> u32 {
    ((hash >> shift) & 0x1F) as u32
}

/// Returns the single-bit mask for the given fragment (0..31).
#[inline]
#[must_use]
pub const fn mask(frag: u32) -> u32 {
    1 << frag
}

/// Returns the compact index of `bit` within `bitmap`.
///
/// Counts the number of set bits below `bit`.
#[inline]
#[must_use]
pub const fn index(bitmap: u32, bit: u32) -> usize {
    (bitmap & (bit - 1)).count_ones() as usize
}

/// Offsets a base index by `n` positions.
#[inline]
#[must_use]
pub const fn offset<T>(base: Idx<T>, n: usize) -> Idx<T> {
    Idx::from_raw(base.into_raw() + n)
}

// ---------------------------------------------------------------------------
// Node accessors
// ---------------------------------------------------------------------------

impl<K, V> Node<K, V> {
    /// Returns the `AdHash` of this node's subtree.
    #[must_use]
    pub const fn adhash(&self) -> u64 {
        match self {
            Self::Inner { adhash, .. } | Self::Collision { adhash, .. } => *adhash,
        }
    }

    /// Returns the number of inline data entries.
    #[must_use]
    pub const fn data_len(&self) -> usize {
        match self {
            Self::Inner { data_map, .. } => data_map.count_ones() as usize,
            Self::Collision { entries_len, .. } => *entries_len as usize,
        }
    }

    /// Returns the number of child subtrees (always 0 for collision nodes).
    #[must_use]
    pub const fn children_len(&self) -> usize {
        match self {
            Self::Inner { node_map, .. } => node_map.count_ones() as usize,
            Self::Collision { .. } => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Manual trait impls — avoid false `K: Trait, V: Trait` bounds.
// Node contains only indices (Copy) and primitives — no actual K/V data.
// ---------------------------------------------------------------------------

impl<K, V> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for Node<K, V> {}

impl<K, V> fmt::Debug for Node<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inner {
                data_map,
                node_map,
                adhash,
                ..
            } => f
                .debug_struct("Inner")
                .field("data_map", &format_args!("{data_map:#034b}"))
                .field("node_map", &format_args!("{node_map:#034b}"))
                .field("adhash", adhash)
                .finish(),
            Self::Collision {
                hash,
                entries_len,
                adhash,
                ..
            } => f
                .debug_struct("Collision")
                .field("hash", hash)
                .field("entries_len", entries_len)
                .field("adhash", adhash)
                .finish(),
        }
    }
}
