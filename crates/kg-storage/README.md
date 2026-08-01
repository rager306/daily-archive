# kg-storage

Universal graph storage abstraction for Rust.

Provides `GraphStore` and `DirectGraphStore` traits, a thread-safe
in-memory `MockGraphStore` for tests, and (in a follow-up phase) the
`SamyamaGraphStore` adapter for the Samyama embedded graph engine.

## Status

**Phase B** (current): skeleton crate with universal traits + error
types + fully functional MockGraphStore. 4 tests.

The Samyama adapter is a stub — the full implementation (~500 lines)
moves here from `da-adapters/src/samyama_graph.rs` in a follow-up
slice once the samyama-sdk API surface is confirmed.

## Usage

```toml
[dependencies]
kg-storage = { path = "crates/kg-storage" }
```

```rust
use kg_storage::{DirectGraphStore, MockGraphStore};

#[tokio::test]
async fn example() {
    let store = MockGraphStore::new();
    let id = store.create_node("Paper").await.unwrap();
    store.set_node_property_string(id, "title", "Hello".to_string()).await.unwrap();
    assert_eq!(store.node_count_total(), 1);
}
```

## Design

- **Zero project-specific dependencies.** This crate does not know
  about `Paper`, `Claim`, or any domain. `Vid` is `String` at the
  storage boundary.
- **MockGraphStore is Clone-able** (Arc-backed inner state) so tests
  can hand one clone to a use case and keep another for inspection.
- See [ADR-050](../../doc/adr/ADR-050-universal-graph-subsystem-kg-crate-family.md)
  for the full architecture decision.

## License

MIT
