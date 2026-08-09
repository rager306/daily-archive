//! kg-storage — Universal graph storage abstraction.
//!
//! Reusable crate providing the `GraphStore` and `DirectGraphStore`
//! traits and a `MockGraphStore` for tests. Zero project-specific
//! dependencies — works against generic node labels and string VID
//! values.
//!
//! See ADR-050 for the full architecture decision.

#[cfg(feature = "algorithms")]
pub mod adapter;
pub mod contract;
pub mod error;
pub mod graph_mode;
pub mod mock;
pub mod mutation;
pub mod traits;

#[cfg(feature = "samyama")]
pub mod samyama_adapter;

pub use error::{GraphResult, GraphStoreError};
pub use graph_mode::{
    GraphMode, GraphModeError, refuse_persistence_path, refuse_persistence_path_from_env,
};
pub use mock::MockGraphStore;
pub use mutation::{
    InstrumentedGraphStore, MutationCounters, MutationDisposition, MutationFamily,
    MutationFamilyCounters,
};
pub use traits::{DirectGraphStore, GraphStore, QueryResult, VectorMetric, VectorSearchResult};

#[cfg(feature = "samyama")]
pub use samyama_adapter::{
    EdgeId, EdgeType, Label, NodeId, PropertyValue, RecordBatch, SamyamaGraphStore, SamyamaStore,
};

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
#[cfg(test)]
pub(crate) fn roundtrip_json<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json1).expect("deserialize failed");
    let json2 = serde_json::to_string(&back).expect("re-serialize failed");
    assert_eq!(json1, json2, "roundtrip JSON mismatch");
}

#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;

    #[test]
    fn roundtrip_query_result() {
        roundtrip_json(&QueryResult {
            columns: vec!["vid".into(), "label".into()],
            records: vec![
                vec![serde_json::json!("abc"), serde_json::json!(42)],
                vec![serde_json::json!("def"), serde_json::json!(7)],
            ],
        });
    }

    #[test]
    fn roundtrip_query_result_empty() {
        roundtrip_json(&QueryResult {
            columns: vec![],
            records: vec![],
        });
    }

    #[test]
    fn roundtrip_vector_search_result() {
        roundtrip_json(&VectorSearchResult {
            vid: "vid:entity:e1".into(),
            score: 0.95,
            properties: serde_json::json!({"label": "GRPO", "type": "Method"}),
        });
    }

    #[test]
    fn roundtrip_vector_metric_all_variants() {
        roundtrip_json(&VectorMetric::Cosine);
        roundtrip_json(&VectorMetric::L2);
        roundtrip_json(&VectorMetric::Dot);
    }
}
