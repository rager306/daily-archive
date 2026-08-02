//! Thin contract runner for Embedder (ADR-051 Contour 4).

use kg_embeddings::contract::embedder_contract_suite;
use kg_embeddings::mock::MockEmbedder;

#[tokio::test]
async fn mock_passes_embedder_contract() {
    let embedder = MockEmbedder::new(8);
    embedder_contract_suite(&embedder).await;
}
