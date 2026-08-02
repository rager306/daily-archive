//! Thin contract runner for SamyamaGraphStore (ADR-051 Contour 4).
//!
//! Same suite as the Mock runner — proves the adapter honors the
//! DirectGraphStore + GraphStore contract. If this test fails, the
//! adapter has a semantic gap that must be fixed before shipping.

#[cfg(feature = "samyama")]
mod inner {
    use kg_storage::SamyamaGraphStore;
    use kg_storage::contract::graph_store_contract_suite;

    #[tokio::test]
    async fn samyama_passes_graph_store_contract() {
        let store = SamyamaGraphStore::new();
        graph_store_contract_suite(&store).await;
    }
}
