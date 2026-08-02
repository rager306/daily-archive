//! Thin contract runner: exercises the public
//! [`kg_storage::contract::graph_store_contract_suite`] against
//! [`MockGraphStore`].
//!
//! The suite itself lives in `src/contract.rs` so Samyama and future
//! adapters can import it without copying (ADR-051 §6 Contour 4).

use kg_storage::contract::graph_store_contract_suite;
use kg_storage::mock::MockGraphStore;

#[tokio::test]
async fn mock_passes_graph_store_contract() {
    let store = MockGraphStore::new();
    graph_store_contract_suite(&store).await;
}
