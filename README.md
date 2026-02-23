# champ-trie

Persistent hash map for Rust based on CHAMP (Compressed Hash-Array Mapped Prefix-tree).

**Canonical form. O(1) structural equality. Arena-backed COW.**

## Why champ-trie?

| Feature | `champ-trie` | `im` | `rpds` | `hash-trie` |
|---------|-------------|------|--------|-------------|
| Algorithm | **CHAMP** | HAMT | HAMT | HAMT |
| Canonical form | **yes** | no | no | no |
| O(1) equality | **yes** (`AdHash`) | no (O(n)) | no (O(n)) | no |
| Inline data in nodes | **yes** | no (leaf only) | no (leaf only) | no |
| Structural sharing | **COW** | COW | COW | COW |
| Node ownership | **Arena** (sole owner) | `Arc<T>` | `Arc<T>` | `Arc<T>` |
| `unsafe` in public API | **none** | yes | no | no |
| Checkpoint/rollback | **yes** | no | no | no |
| Thread-safe variant | **yes** (`ChampMapSync`) | yes (`Arc`) | yes (`Arc`) | yes (`Arc`) |

Existing persistent map crates ([`im`](https://crates.io/crates/im),
[`rpds`](https://crates.io/crates/rpds)) use classic HAMT. `champ-trie`
implements the CHAMP algorithm (Steindorfer & Vinju, OOPSLA 2015), which
provides three properties HAMT lacks:

- **Canonical form**: same set of key-value pairs always produces the same
  trie structure, regardless of insertion order. Classic HAMT does not
  guarantee this — the structure depends on history.
- **`AdHash`**: O(1) structural equality via incrementally maintained hash.
  Mathematically a group homomorphism `(P_fin(K x V), triangle) -> (Z/2^64, +)`.
  No need to traverse the entire trie for equality checks.
- **Compact layout**: key-value pairs stored inline in interior nodes at the
  shallowest unique level, reducing pointer chasing and improving cache
  locality compared to HAMT's leaf-only storage.

## Arena ownership — why not `Rc`/`Arc`?

Persistent data structures require structural sharing: multiple versions of
the map share interior nodes. The standard approach (`im`, `rpds`) uses
`Arc<Node>` — reference counting with shared ownership.

`champ-trie` uses [`safe-bump`](https://crates.io/crates/safe-bump) arenas
instead. The arena is owned by the map — no manual arena management:

- **Sole ownership**: arena owns all nodes. No reference counting. No shared
  ownership.
- **Bump allocation**: O(1) allocation. No per-node heap overhead.
- **Checkpoint/rollback**: speculative mutations — allocate tentatively,
  validate invariants, then keep or discard.
- **Batch deallocation**: drop the map to free everything at once. No
  cascading `Arc::drop` traversals.

The tradeoff: dead nodes from COW operations remain in the arena until the
map is dropped. Mitigated by checkpoint/rollback for speculative operations
and by compaction for long-lived maps.

## Usage

```rust
use champ_trie::ChampMap;

// Standard mutable API — arena managed internally
let mut map = ChampMap::new();
map.insert("alice", 1);
map.insert("bob", 2);

assert_eq!(map.get(&"alice"), Some(&1));
assert_eq!(map.get(&"bob"), Some(&2));
assert_eq!(map.len(), 2);

map.remove(&"alice");
assert_eq!(map.len(), 1);

// O(1) equality via AdHash — canonical form guarantees
// same contents → same adhash, regardless of insertion order
let mut a = ChampMap::new();
a.insert("x", 1);
a.insert("y", 2);

let mut b = ChampMap::new();
b.insert("y", 2);
b.insert("x", 1);

assert_eq!(a.adhash(), b.adhash()); // same contents → same AdHash
assert_eq!(a.len(), b.len());
```

```rust
use champ_trie::ChampMapSync;

// Thread-safe variant — same API, SharedArena backend
let mut map = ChampMapSync::new();
map.insert("key", 42);
// Send + Sync, wait-free reads
```

## Choosing a backend

| Type | Backend | Threading | Overhead |
|------|---------|-----------|----------|
| `ChampMap<K, V>` | `Arena<T>` | single-thread | zero |
| `ChampMapSync<K, V>` | `SharedArena<T>` | `Send + Sync` | OnceLock per slot |

Same algorithm, same guarantees. Choose by type.

## Design

Each CHAMP node contains two bitmaps over 32 positions:

```text
data_map : which positions hold inline (key, value) pairs
node_map : which positions hold child subtree references
invariant: data_map & node_map == 0
```

Entries are stored at the **shallowest level** where their hash prefix is
unique. When a collision occurs, both entries migrate to a deeper subtree.
When deletion reduces a subtree to a single entry, it migrates back to the
parent (inlining). This bidirectional migration maintains canonical form.

### Complexity

| Operation | Time | Notes |
|-----------|------|-------|
| `get` | O(log₃₂ n) | depth ≤ 13 for 64-bit hash |
| `insert` | O(log₃₂ n) | COW path copy |
| `remove` | O(log₃₂ n) | COW path copy + inlining |
| `adhash` | O(1) | incrementally maintained |
| `checkpoint` | O(1) | saves three arena cursors |
| `rollback` | O(k) | k = items allocated since checkpoint |
| `iter` | O(n) | DFS collect |
| `len` | O(1) | tracked in map |

### Trait bounds

- Read operations: `K: Hash + Eq`
- Write operations: `K: Hash + Eq + Clone, V: Hash + Clone`

`Clone` is required for COW path-copy — entries must be cloned into
new arena slots when interior nodes are copied.
`V: Hash` is required for `AdHash` (O(1) structural equality).

### Standard traits

`ChampMap<K, V>` and `ChampMapSync<K, V>` implement `Debug`, `Default`,
`FromIterator<(K, V)>`, `Extend<(K, V)>`, `Index<&K>`, and
`IntoIterator` for `&map` (yields `(&K, &V)`).

## Limitations

- **Arena waste**: COW path copying leaves dead nodes in the arena.
  Mitigated by checkpoint/rollback for speculative operations.
- **Hash collisions**: true 64-bit hash collisions (probability ~1/2⁶⁴)
  are handled correctly via collision nodes with linear search by `Eq`.
  Both entries are preserved — no data loss.

## References

- Steindorfer & Vinju, 2015 — "Optimizing Hash-Array Mapped Tries
  for Fast and Lean Immutable JVM Collections", OOPSLA 2015
- Bagwell, 2001 — "Ideal Hash Trees"

## License

Apache License 2.0. See [LICENSE](LICENSE).
