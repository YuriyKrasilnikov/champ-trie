//! Non-functional requirement tests: asymptotic complexity, memory, COW sharing.
//!
//! These tests verify quantitative properties of the CHAMP trie:
//! - O(log₃₂ n) get/insert/remove
//! - O(n) total memory
//! - O(D) allocations per COW mutation
//! - O(1) checkpoint/rollback
//! - O(n) iteration

use std::hint::black_box;
use std::time::Instant;

/// Measures wall-clock time of a closure in nanoseconds.
fn measure_ns<F: FnMut()>(mut f: F) -> u64 {
    let start = Instant::now();
    f();
    start.elapsed().as_nanos() as u64
}

/// Runs `f` multiple times and returns median time in nanoseconds.
fn median_ns<F: FnMut()>(iterations: u32, mut f: F) -> u64 {
    let mut times: Vec<u64> = (0..iterations)
        .map(|_| measure_ns(&mut f))
        .collect();
    times.sort_unstable();
    times[times.len() / 2]
}

macro_rules! nfr_tests {
    ($mod_name:ident, $map_type:ty, iter_bound = $iter_bound:expr) => {
        mod $mod_name {
            use super::*;

            // =================================================================
            // Asymptotic: O(log₃₂ n) operations
            // =================================================================

            /// get time grows sublinearly with map size.
            ///
            /// log₃₂(1_000) ≈ 2.0, log₃₂(100_000) ≈ 3.3
            /// So 100x more entries should yield < 2x slower gets.
            /// We use 5x headroom for CI noise.
            #[test]
            fn get_sublinear() {
                let small = build_map::<$map_type>(1_000);
                let large = build_map::<$map_type>(100_000);

                let t_small = median_ns(5, || {
                    for i in 0_u64..1_000 {
                        black_box(small.get(&i));
                    }
                });
                let t_large = median_ns(5, || {
                    for i in 0_u64..1_000 {
                        black_box(large.get(&i));
                    }
                });

                let ratio = t_large as f64 / t_small as f64;
                assert!(
                    ratio < 5.0,
                    "get ratio {ratio:.2}x exceeds 5x bound (small={t_small}ns, large={t_large}ns)"
                );
            }

            /// insert time grows sublinearly with map size.
            ///
            /// Uses checkpoint/rollback to isolate mutation measurement
            /// from O(n) map construction.
            #[test]
            fn insert_sublinear() {
                let mut small = build_map::<$map_type>(1_000);
                let cp_small = small.checkpoint();
                let t_small = median_ns(5, || {
                    for i in 1_000_u64..2_000 {
                        small.insert(i, i);
                    }
                    black_box(&small);
                    small.rollback(cp_small);
                });

                let mut large = build_map::<$map_type>(100_000);
                let cp_large = large.checkpoint();
                let t_large = median_ns(5, || {
                    for i in 100_000_u64..101_000 {
                        large.insert(i, i);
                    }
                    black_box(&large);
                    large.rollback(cp_large);
                });

                let ratio = t_large as f64 / t_small as f64;
                assert!(
                    ratio < 5.0,
                    "insert ratio {ratio:.2}x exceeds 5x bound (small={t_small}ns, large={t_large}ns)"
                );
            }

            /// remove time grows sublinearly with map size.
            ///
            /// Uses checkpoint/rollback to isolate mutation measurement.
            #[test]
            fn remove_sublinear() {
                let mut small = build_map::<$map_type>(2_000);
                let cp_small = small.checkpoint();
                let t_small = median_ns(5, || {
                    for i in 0_u64..1_000 {
                        small.remove(&i);
                    }
                    black_box(&small);
                    small.rollback(cp_small);
                });

                let mut large = build_map::<$map_type>(101_000);
                let cp_large = large.checkpoint();
                let t_large = median_ns(5, || {
                    for i in 0_u64..1_000 {
                        large.remove(&i);
                    }
                    black_box(&large);
                    large.rollback(cp_large);
                });

                let ratio = t_large as f64 / t_small as f64;
                assert!(
                    ratio < 5.0,
                    "remove ratio {ratio:.2}x exceeds 5x bound (small={t_small}ns, large={t_large}ns)"
                );
            }

            // =================================================================
            // Memory: O(n) total arena size
            // =================================================================

            /// Total arena allocations grow linearly with entry count.
            ///
            /// For a CHAMP trie with n entries:
            /// - entries arena: exactly n live entries (plus COW dead copies)
            /// - nodes arena: O(n) nodes
            /// - children arena: O(n) child pointers
            ///
            /// Total should be bounded by c*n for reasonable c.
            #[test]
            fn memory_linear() {
                let sizes = [1_000_u64, 10_000, 50_000];
                let mut ratios = Vec::new();

                for &n in &sizes {
                    let map = build_map::<$map_type>(n);
                    let (nodes, entries, children) = map.arena_len();
                    let total = nodes + entries + children;
                    let ratio = total as f64 / n as f64;
                    ratios.push(ratio);
                }

                // Ratios should be roughly constant (within 2x of each other).
                let min = ratios.iter().copied().fold(f64::INFINITY, f64::min);
                let max = ratios.iter().copied().fold(0.0_f64, f64::max);
                assert!(
                    max / min < 2.0,
                    "memory ratio not constant: ratios={ratios:?} (min={min:.2}, max={max:.2})"
                );
            }

            // =================================================================
            // COW: O(D) allocations per single mutation
            // =================================================================

            /// Single insert allocates O(D) new nodes, not O(n).
            ///
            /// D = max depth = 13. Each insert path-copies at most D nodes,
            /// plus 1 entry, plus up to D children pointers.
            /// Total delta should be bounded by a small constant.
            #[test]
            fn cow_single_insert() {
                let mut map = build_map::<$map_type>(100_000);
                let before = map.arena_len();
                map.insert(999_999, 999_999);
                let after = map.arena_len();

                let delta_nodes = after.0 - before.0;
                let delta_entries = after.1 - before.1;
                let delta_children = after.2 - before.2;
                let total_delta = delta_nodes + delta_entries + delta_children;

                // Max depth 13, so path-copy creates at most ~13 nodes,
                // ~13 entry arrays, ~13 children arrays.
                // With flattening, actual delta should be much less.
                // Use generous bound of 200 total items.
                assert!(
                    total_delta < 200,
                    "single insert allocated {total_delta} items \
                     (nodes=+{delta_nodes}, entries=+{delta_entries}, children=+{delta_children})"
                );
            }

            /// Single remove allocates O(D) new nodes, not O(n).
            #[test]
            fn cow_single_remove() {
                let mut map = build_map::<$map_type>(100_000);
                let before = map.arena_len();
                map.remove(&50_000);
                let after = map.arena_len();

                let delta_nodes = after.0 - before.0;
                let delta_entries = after.1 - before.1;
                let delta_children = after.2 - before.2;
                let total_delta = delta_nodes + delta_entries + delta_children;

                assert!(
                    total_delta < 200,
                    "single remove allocated {total_delta} items \
                     (nodes=+{delta_nodes}, entries=+{delta_entries}, children=+{delta_children})"
                );
            }

            // =================================================================
            // Checkpoint: O(1) time and space
            // =================================================================

            /// Checkpoint creation time is constant regardless of map size.
            #[test]
            fn checkpoint_constant_time() {
                let small = build_map::<$map_type>(1_000);
                let large = build_map::<$map_type>(100_000);

                let t_small = median_ns(11, || {
                    black_box(small.checkpoint());
                });
                let t_large = median_ns(11, || {
                    black_box(large.checkpoint());
                });

                // Both should be near-instant. Allow 10x for noise.
                let ratio = if t_small == 0 { 1.0 } else { t_large as f64 / t_small as f64 };
                assert!(
                    ratio < 10.0,
                    "checkpoint ratio {ratio:.2}x exceeds 10x (small={t_small}ns, large={t_large}ns)"
                );
            }

            /// Checkpoint is zero-allocation (arena sizes unchanged).
            #[test]
            fn checkpoint_zero_alloc() {
                let map = build_map::<$map_type>(10_000);
                let before = map.arena_len();
                let _cp = map.checkpoint();
                let after = map.arena_len();
                assert_eq!(before, after, "checkpoint should not allocate");
            }

            // =================================================================
            // Iter: O(n) time
            // =================================================================

            /// Iteration time scales linearly with entry count.
            #[test]
            fn iter_linear() {
                let small = build_map::<$map_type>(10_000);
                let large = build_map::<$map_type>(100_000);

                let t_small = median_ns(5, || {
                    let mut count = 0_u64;
                    for (k, v) in small.iter() {
                        count += black_box(*k) + black_box(*v);
                    }
                    black_box(count);
                });
                let t_large = median_ns(5, || {
                    let mut count = 0_u64;
                    for (k, v) in large.iter() {
                        count += black_box(*k) + black_box(*v);
                    }
                    black_box(count);
                });

                // 10x entries → time should be ~10x in theory.
                // Debug mode inflates the ratio: no inlining, bounds checks,
                // cache pressure from larger working sets (1.6MB at 100K).
                // SharedArena (chunked) adds OnceLock indirection on top.
                // Bound catches O(n²) regression (would be 100x+), not exact linearity.
                let bound: f64 = $iter_bound;
                let ratio = t_large as f64 / t_small as f64;
                assert!(
                    ratio < bound,
                    "iter ratio {ratio:.2}x exceeds {bound}x for 10x entries \
                     (small={t_small}ns, large={t_large}ns)"
                );
                // Also verify it's not sublinear (at least 2x for 10x entries).
                assert!(
                    ratio > 2.0,
                    "iter suspiciously fast: ratio {ratio:.2}x for 10x entries — \
                     possible dead code elimination"
                );
            }

            /// Iter yields exactly `len()` entries.
            #[test]
            fn iter_count_matches_len() {
                for &n in &[0_u64, 1, 10, 100, 1_000, 10_000] {
                    let map = build_map::<$map_type>(n);
                    assert_eq!(
                        map.iter().count(),
                        map.len(),
                        "iter count != len for n={n}"
                    );
                }
            }

            // =================================================================
            // Helper
            // =================================================================

            fn build_map<M>(n: u64) -> M
            where
                M: Default + MapInsert,
            {
                let mut map = M::default();
                for i in 0..n {
                    map.map_insert(i, i);
                }
                map
            }
        }
    };
}

/// Trait to abstract over insert for both map types.
trait MapInsert {
    fn map_insert(&mut self, key: u64, value: u64);
}

impl MapInsert for crate::ChampMap<u64, u64> {
    fn map_insert(&mut self, key: u64, value: u64) {
        self.insert(key, value);
    }
}

impl MapInsert for crate::ChampMapSync<u64, u64> {
    fn map_insert(&mut self, key: u64, value: u64) {
        self.insert(key, value);
    }
}

nfr_tests!(single, crate::ChampMap<u64, u64>, iter_bound = 60.0);
nfr_tests!(sync, crate::ChampMapSync<u64, u64>, iter_bound = 60.0);
