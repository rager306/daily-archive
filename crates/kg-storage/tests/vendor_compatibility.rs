//! Existing port semantics against the pinned vendor and the state-based fake.

#![cfg(feature = "samyama")]

use kg_storage::samyama_adapter::{EdgeId, NodeId, PropertyValue};
use kg_storage::snapshot_identity::{SEMANTIC_SNAPSHOT_ALGORITHM_V2, fingerprint_snapshot};
use kg_storage::{DirectGraphStore, GraphStore, MockGraphStore, SamyamaGraphStore, VectorMetric};

async fn vector_write_contract<T: GraphStore + DirectGraphStore>(store: &T) {
    store
        .create_vector_index("Paper", "embedding", 2, VectorMetric::Cosine)
        .await
        .unwrap();
    let first = store.create_node("Paper").await.unwrap();
    let second = store.create_node("Paper").await.unwrap();
    let old = vec![1.0, 0.0];
    let new = vec![0.0, 1.0];
    store
        .add_vector("Paper", "embedding", first, old.clone())
        .await
        .unwrap();
    store
        .add_vector("Paper", "embedding", first, old.clone())
        .await
        .unwrap();
    store
        .add_vector("Paper", "embedding", second, old.clone())
        .await
        .unwrap();
    let before = store
        .vector_search_direct("Paper", "embedding", &old, 10)
        .await
        .unwrap();
    assert_eq!(before.len(), 2);
    assert_eq!(before.iter().filter(|(id, _)| *id == first).count(), 1);
    store
        .replace_vector("Paper", "embedding", first, new.clone())
        .await
        .unwrap();
    store
        .replace_vector("Paper", "embedding", first, new.clone())
        .await
        .unwrap();
    assert!(
        store
            .replace_vector("Paper", "embedding", u64::MAX, new.clone())
            .await
            .is_err()
    );
    assert!(
        store
            .replace_vector("Paper", "absent", first, new.clone())
            .await
            .is_err()
    );
    let current = store
        .vector_search_direct("Paper", "embedding", &new, 10)
        .await
        .unwrap();
    assert_eq!(current.len(), 2);
    assert_eq!(current[0].0, first);
    assert_eq!(current.iter().filter(|(id, _)| *id == first).count(), 1);
}

#[tokio::test]
async fn mock_and_samyama_pass_the_same_vector_write_contract() {
    vector_write_contract(&MockGraphStore::new()).await;
    vector_write_contract(&SamyamaGraphStore::new()).await;
}

#[tokio::test]
async fn embedded_conflicts_and_dimension_errors_leave_the_vector_unchanged() {
    let store = SamyamaGraphStore::new();
    store
        .create_vector_index("Paper", "embedding", 2, VectorMetric::Cosine)
        .await
        .unwrap();
    let node = store.create_node("Paper").await.unwrap();
    let old = vec![1.0, 0.0];
    store
        .add_vector("Paper", "embedding", node, old.clone())
        .await
        .unwrap();
    assert!(
        store
            .add_vector("Paper", "embedding", node, vec![0.0, 1.0])
            .await
            .is_err()
    );
    assert!(
        store
            .replace_vector("Paper", "embedding", node, vec![1.0])
            .await
            .is_err()
    );
    let hits = DirectGraphStore::vector_search_direct(&store, "Paper", "embedding", &old, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, node);
    assert!(hits[0].1.abs() < 1e-6);
}

#[tokio::test]
async fn column_values_override_stale_rows_and_survive_snapshot_restore() {
    let store = SamyamaGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(a, "vid", "a".into())
        .await
        .unwrap();
    store
        .set_node_property_string(b, "vid", "b".into())
        .await
        .unwrap();
    let edge = store.create_edge(a, b, "CITES").await.unwrap();
    {
        let mut raw = store.store_write().await;
        raw.get_node_mut(NodeId(a))
            .unwrap()
            .set_property("title", PropertyValue::String("stale".into()));
        raw.set_column_property(NodeId(a), "title", PropertyValue::String("current".into()));
        raw.get_edge_properties_mut(EdgeId(edge))
            .unwrap()
            .insert("valid_at".into(), PropertyValue::String("stale".into()));
    }
    store
        .set_edge_property_string(edge, "valid_at", "current")
        .await
        .unwrap();
    store
        .set_edge_property_float(edge, "weight", 0.25)
        .await
        .unwrap();
    assert_eq!(
        store.get_node_property_string(a, "title").await.as_deref(),
        Some("current")
    );
    assert_eq!(
        store
            .get_edge_property_string(edge, "valid_at")
            .await
            .as_deref(),
        Some("current")
    );
    assert_eq!(
        store.get_edge_property_float(edge, "weight").await,
        Some(0.25)
    );
    let bytes = store.export_snapshot().await.unwrap();
    let identity = fingerprint_snapshot(&bytes).unwrap();
    assert_eq!(identity.semantic_algorithm, SEMANTIC_SNAPSHOT_ALGORITHM_V2);
    let restored = SamyamaGraphStore::new();
    restored.import_snapshot(&bytes).await.unwrap();
    let a = restored
        .find_node_by_string_property("Paper", "vid", "a")
        .await
        .unwrap();
    let b = restored
        .find_node_by_string_property("Paper", "vid", "b")
        .await
        .unwrap();
    let edge = restored.get_edges_between(a, b, "CITES").await[0].0;
    assert_eq!(
        restored
            .get_node_property_string(a, "title")
            .await
            .as_deref(),
        Some("current")
    );
    assert_eq!(
        restored
            .get_edge_property_string(edge, "valid_at")
            .await
            .as_deref(),
        Some("current")
    );
    assert_eq!(
        restored.get_edge_property_float(edge, "weight").await,
        Some(0.25)
    );
    let reexport = restored.export_snapshot().await.unwrap();
    assert_eq!(
        fingerprint_snapshot(&reexport).unwrap().semantic_sha256,
        identity.semantic_sha256
    );
}

#[tokio::test]
async fn declared_indexes_and_hierarchy_survive_a_real_snapshot_roundtrip() {
    use kg_storage::samyama_adapter::{EdgeType, Label};
    use samyama::index::catalog::IndexDefinition;
    use samyama::index::hierarchy::{HierarchySpec, RollupOp, RollupValue};
    use samyama::vector::DistanceMetric;
    use samyama::vector::index::Quantization;

    let store = SamyamaGraphStore::new();
    {
        let mut raw = store.store_write().await;
        let root = raw.create_node("Class");
        raw.set_node_property("default", root, "vid", "root")
            .unwrap();
        for i in 1..=4i64 {
            let leaf = raw.create_node("Drug");
            raw.set_node_property("default", leaf, "vid", format!("drug-{i}"))
                .unwrap();
            raw.set_node_property("default", leaf, "title", format!("compound {i}"))
                .unwrap();
            raw.set_node_property("default", leaf, "units", PropertyValue::Integer(i))
                .unwrap();
            raw.set_node_property(
                "default",
                leaf,
                "embedding",
                PropertyValue::Vector(vec![1.0, i as f32]),
            )
            .unwrap();
            raw.create_edge(leaf, root, "IS_A").unwrap();
        }
        raw.create_unique_constraint(&Label::new("Drug"), "vid")
            .unwrap();
        raw.create_fulltext_index("titles", "Drug", "title");
        raw.create_vector_index_named(
            Some("embeddings"),
            "Drug",
            "embedding",
            2,
            DistanceMetric::Cosine,
            Quantization::None,
        )
        .unwrap();
        for leaf in raw.node_ids_by_label(&Label::new("Drug"), None) {
            let PropertyValue::Vector(vector) = raw.node_property(leaf, "embedding").unwrap()
            else {
                panic!("vector property")
            };
            raw.vector_index
                .add_vector("Drug", "embedding", leaf, &vector)
                .unwrap();
        }
        let hierarchy = std::sync::Arc::clone(&raw.hierarchy_index);
        hierarchy
            .create(
                &raw,
                HierarchySpec::new("taxonomy", vec![EdgeType::new("IS_A")]).with_measure(
                    None,
                    "units",
                    vec![RollupOp::Sum, RollupOp::Count],
                ),
            )
            .unwrap();
    }
    let expected = vec![
        IndexDefinition::Property {
            label: "Drug".into(),
            property: "vid".into(),
        },
        IndexDefinition::UniqueConstraint {
            label: "Drug".into(),
            property: "vid".into(),
        },
        IndexDefinition::FullText {
            name: "titles".into(),
            label: "Drug".into(),
            property: "title".into(),
        },
        IndexDefinition::Vector {
            name: Some("embeddings".into()),
            label: "Drug".into(),
            property: "embedding".into(),
            dimensions: 2,
            metric: DistanceMetric::Cosine,
            quantization: Quantization::None,
        },
    ];
    let bytes = store.export_snapshot().await.unwrap();
    let identity = fingerprint_snapshot(&bytes).unwrap();
    assert_eq!(identity.semantic_algorithm, SEMANTIC_SNAPSHOT_ALGORITHM_V2);
    let restored = SamyamaGraphStore::new();
    restored.import_snapshot(&bytes).await.unwrap();
    {
        let raw = restored.store_read().await;
        let catalog = raw.index_catalog();
        assert_eq!(catalog.definitions.len(), expected.len());
        for definition in &expected {
            assert!(catalog.definitions.contains(definition), "{definition:?}");
        }
        assert_eq!(
            raw.resolve_vector_index("embeddings"),
            Some(("Drug".into(), "embedding".into()))
        );
        let hierarchy = raw.hierarchy_index.get("taxonomy").unwrap();
        let guard = hierarchy.read().unwrap();
        assert!(guard.usable());
        assert_eq!(guard.spec.edge_types, vec![EdgeType::new("IS_A")]);
        assert_eq!(guard.spec.measure.as_ref().unwrap().property, "units");
        let index = guard.index.as_ref().unwrap();
        let root = (0..index.poset().n() as u32)
            .find(|&id| index.descendant_count(id) == 5)
            .unwrap();
        assert_eq!(
            index.rollup(root, RollupOp::Sum),
            Some(RollupValue::Int(10))
        );
        assert_eq!(
            index.rollup(root, RollupOp::Count),
            Some(RollupValue::Int(5))
        );
    }
    let hits =
        DirectGraphStore::vector_search_direct(&restored, "Drug", "embedding", &[1.0, 1.0], 10)
            .await
            .unwrap();
    assert_eq!(hits.len(), 4);
    let first = restored
        .find_node_by_string_property("Drug", "vid", "drug-1")
        .await
        .unwrap();
    assert_eq!(hits[0].0, first);
    let reexport = restored.export_snapshot().await.unwrap();
    assert_eq!(
        fingerprint_snapshot(&reexport).unwrap().semantic_sha256,
        identity.semantic_sha256
    );
}

#[cfg(feature = "algorithms")]
#[tokio::test]
async fn vendor_edge_weights_reach_the_actual_graph_view() {
    let store = SamyamaGraphStore::new();
    let a = store.create_node("Paper").await.unwrap();
    let b = store.create_node("Paper").await.unwrap();
    let edge = store.create_edge(a, b, "CITES").await.unwrap();
    store
        .set_edge_property_float(edge, "weight", 0.375)
        .await
        .unwrap();
    let view =
        kg_storage::adapter::graph_view_from(&store, Some("Paper"), Some("CITES"), Some("weight"))
            .await
            .unwrap();
    assert_eq!(view.out_targets.len(), 1);
    assert_eq!(view.weights.as_deref(), Some([0.375].as_slice()));
}
