//! `AdHash` — incremental structural hashing.
//!
//! Computes `φ(S) = Σ f(k, v)` over all entries using wrapping arithmetic.
//! Two mixing seeds prevent degeneration when `hash(v) = 0`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// First mixing seed (golden ratio constant).
const SEED_1: u64 = 0x9E37_79B9_7F4A_7C15;

/// Second mixing seed (large prime).
const SEED_2: u64 = 0x517C_C1B7_2722_0A95;

/// Computes the 64-bit hash of a value using the standard hasher.
#[must_use]
pub fn hash_one<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Computes the `AdHash` contribution of a single entry.
///
/// `f(k, v) = key_hash · SEED₁ ⊕ value_hash · SEED₂`
#[must_use]
pub const fn entry_adhash(key_hash: u64, value_hash: u64) -> u64 {
    key_hash.wrapping_mul(SEED_1) ^ value_hash.wrapping_mul(SEED_2)
}
