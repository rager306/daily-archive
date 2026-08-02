# kg-storage

Universal graph storage abstraction for Rust.

Provides `GraphStore` and `DirectGraphStore` traits, a thread-safe
in-memory `MockGraphStore` for tests, the `SamyamaGraphStore` adapter
for the Samyama embedded graph engine, and a **port contract suite**
(ADR-051 Contour 4) that every adapter must pass.

## Status

**ADR-051 Contour 4** (current): public contract suite in
`src/contract.rs` — `graph_store_contract_suite::<T>()` — run against
every `DirectGraphStore + GraphStore` implementation. The Mock adapter
passes; Samyama is wired but edge-property support is pending (S02).

12 unit tests + 1 contract integration test.

## Contract suite (ADR-051 §6)

```rust
use kg_storage::contract::graph_store_contract_suite;
use kg_storage::mock::MockGraphStore;

#[tokio::test]
async fn mock_passes_graph_store_contract() {
    let store = MockGraphStore::new();
    graph_store_contract_suite(&store).await;
}
```

The suite covers the full ADR-051 GraphStore checklist:

- unique node/edge IDs from `create_node` / `create_edge`
- set/get string/int/bool/float node properties
- missing property → `None` (not-found semantics)
- outgoing/incoming edges
- node_count / edge_count deltas
- `find_node_by_string_property` label isolation (Paper vs Citation)
- `get_nodes_by_label` filtering
- `get_edges_between` type filter
- edge property set/get via `set_edge_property_string_v2` + `get_edge_property_string`
- `health() == Ok(true)`
- `export_snapshot` Ok non-empty

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
  storage boundary; `da_domain::vid::Vid` is a type alias.
- **Hexagonal port.** `da_ports::graph_store` re-exports these traits
  so application code depends on `da-ports`, not `kg-storage` directly.
- **Contract-first.** The contract suite is the source of truth for
  adapter correctness — not the Mock, not the adapter's own tests.

## Modules

| Module | Purpose |
|--------|---------|
| `traits` | `GraphStore` + `DirectGraphStore` trait definitions |
| `mock` | `MockGraphStore` — thread-safe in-memory test double |
| `contract` | `graph_store_contract_suite` — ADR-051 Contour 4 |
| `samyama_adapter` | `SamyamaGraphStore` — Samyama embedded client adapter |
| `error` | `GraphStoreError` + `GraphResult` |
