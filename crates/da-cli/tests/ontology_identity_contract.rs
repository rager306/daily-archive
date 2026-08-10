use std::collections::BTreeSet;
use std::path::Path;

use da_cli::gold_graph::{
    ensure_identity_indexes, ensure_property_indexes, load_identity_indexes, load_property_indexes,
    validate_identity_values,
};
use kg_ontology::OntologyRegistry;
use kg_storage::{DirectGraphStore, GraphStore, mock::MockGraphStore};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn project_policy_covers_every_registered_vid_property() {
    let root = root();
    let registry = OntologyRegistry::load_from_dir(root.join("data/ontology")).unwrap();
    let policy = load_identity_indexes(
        &root.join("data/ontology/identity_indexes.yaml"),
        &root.join("data/ontology/node_schemas.yaml"),
    )
    .unwrap();
    let keys = policy
        .indexes()
        .iter()
        .map(|index| (index.label.as_str(), index.property.as_str()))
        .collect::<BTreeSet<_>>();
    for label in registry.labels_owned() {
        let schema = registry.get_schema(&label).unwrap();
        let has_vid = schema
            .required_fields
            .iter()
            .chain(&schema.optional_fields)
            .any(|field| field.name == "vid");
        if has_vid {
            assert!(
                keys.contains(&(label.as_str(), "vid")),
                "missing {label}.vid"
            );
        }
    }
}

#[test]
fn policy_loaders_reject_identity_property_confusion() {
    let root = root();
    assert!(
        load_identity_indexes(
            &root.join("data/ontology/property_indexes.yaml"),
            &root.join("data/ontology/node_schemas.yaml"),
        )
        .is_err()
    );
    assert!(
        load_property_indexes(
            &root.join("data/ontology/identity_indexes.yaml"),
            &root.join("data/ontology/node_schemas.yaml"),
        )
        .is_err()
    );
}

#[tokio::test]
async fn ensure_is_idempotent_and_does_not_mutate_graph_values() {
    let root = root();
    let policy = load_identity_indexes(
        &root.join("data/ontology/identity_indexes.yaml"),
        &root.join("data/ontology/node_schemas.yaml"),
    )
    .unwrap();
    let property_policy = load_property_indexes(
        &root.join("data/ontology/property_indexes.yaml"),
        &root.join("data/ontology/node_schemas.yaml"),
    )
    .unwrap();
    let property_keys = property_policy
        .indexes()
        .iter()
        .map(|index| index.key())
        .collect::<BTreeSet<_>>();
    for required in [
        "Category.code",
        "ConceptCluster.cluster_type",
        "Entity.entity_type",
        "Institution.openalex_id",
        "Paper.openalex_id",
        "SchedulerTask.arxiv_id",
        "Section.path_id",
        "Section.work_vid",
        "Topic.label",
    ] {
        assert!(property_keys.contains(required), "missing {required}");
    }
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "vid", "vid:paper:fixed".into())
        .await
        .unwrap();
    store
        .set_node_property_string(paper, "arxiv_id", "2401.00001".into())
        .await
        .unwrap();
    let before = store.export_snapshot().await.unwrap();
    validate_identity_values(&store, &policy).await.unwrap();
    assert_eq!(before, store.export_snapshot().await.unwrap());
    ensure_identity_indexes(&store, &policy).await.unwrap();
    ensure_identity_indexes(&store, &policy).await.unwrap();
    ensure_property_indexes(&store, &property_policy)
        .await
        .unwrap();
    ensure_property_indexes(&store, &property_policy)
        .await
        .unwrap();
    let after = store.export_snapshot().await.unwrap();
    assert_eq!(before, after);
    assert_eq!(
        store
            .get_node_property_string(paper, "vid")
            .await
            .as_deref(),
        Some("vid:paper:fixed")
    );
}

#[tokio::test]
async fn missing_required_identity_fails_during_plan_without_graph_mutation() {
    let root = root();
    let policy = load_identity_indexes(
        &root.join("data/ontology/identity_indexes.yaml"),
        &root.join("data/ontology/node_schemas.yaml"),
    )
    .unwrap();
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "vid", "vid:paper:only".into())
        .await
        .unwrap();
    let before = store.export_snapshot().await.unwrap();
    let error = validate_identity_values(&store, &policy).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing identity Paper.arxiv_id")
    );
    assert_eq!(before, store.export_snapshot().await.unwrap());
}

#[test]
fn accepted_vid_bytes_and_normalisation_are_frozen() {
    assert_eq!(
        da_domain::vid::paper_vid("1206.6423"),
        "258ee2de9f62bbf3bff1353831bee7887271130f8f2108efed348be917076d6a"
    );
    assert_eq!(
        da_domain::vid::paper_vid("arxiv:1206.6423"),
        da_domain::vid::paper_vid("1206.6423")
    );
    assert_eq!(
        da_domain::vid::author_vid(" Alice Smith "),
        da_domain::vid::author_vid("alice smith")
    );
    assert_eq!(
        da_domain::vid::reference_vid(" Paper A "),
        da_domain::vid::reference_vid("paper a")
    );
}
