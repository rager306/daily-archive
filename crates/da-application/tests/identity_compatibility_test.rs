use da_application::identity_compatibility::{
    IdentityCompatibilityUseCase, IdentityCompatibilityVerdict,
};
use kg_ontology::{IdentityIndexPolicy, OntologyRegistry};
use kg_storage::{DirectGraphStore, GraphStore, mock::MockGraphStore};

fn registry() -> OntologyRegistry {
    OntologyRegistry::load_from_str(
        r#"
node_schemas:
  - label: Paper
    required_fields:
      - {name: vid, field_type: String}
      - {name: arxiv_id, field_type: String}
  - label: Section
    required_fields:
      - {name: vid, field_type: String}
"#,
        "nodes.yaml",
    )
    .unwrap()
}

fn policy() -> IdentityIndexPolicy {
    IdentityIndexPolicy::parse_and_validate(
        br#"
schema_version: ontology-v2-identity-indexes.v1
indexes:
  - {label: Paper, property: arxiv_id, identity: true}
  - {label: Paper, property: vid, identity: true}
  - {label: Section, property: vid, identity: true}
"#,
        &registry(),
    )
    .unwrap()
}

#[tokio::test]
async fn compatible_present_and_unobserved_indexes_are_distinguished() {
    let store = MockGraphStore::new();
    for (vid, arxiv) in [("p1", "2401.00001"), ("p2", "2401.00002")] {
        let node = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(node, "vid", vid.into())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "arxiv_id", arxiv.into())
            .await
            .unwrap();
    }
    let before = store.export_snapshot().await.unwrap();
    let report = IdentityCompatibilityUseCase::new(&store)
        .audit(
            &policy(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .await
        .unwrap();
    let after = store.export_snapshot().await.unwrap();
    assert_eq!(before, after, "identity audit must not mutate graph state");
    assert_eq!(report.verdict, IdentityCompatibilityVerdict::Incomplete);
    assert_eq!(report.indexes[0].status.as_str(), "compatible");
    assert_eq!(report.indexes[1].status.as_str(), "compatible");
    assert_eq!(report.indexes[2].status.as_str(), "unobserved");
    assert!(report.validate().is_ok());
    assert!(!report.import_eligible);
}

#[tokio::test]
async fn missing_and_duplicate_identity_values_fail_semantically() {
    let store = MockGraphStore::new();
    let first = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(first, "vid", "duplicate".into())
        .await
        .unwrap();
    let second = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(second, "vid", "duplicate".into())
        .await
        .unwrap();
    store
        .set_node_property_string(second, "arxiv_id", "2401.00002".into())
        .await
        .unwrap();

    let report = IdentityCompatibilityUseCase::new(&store)
        .audit(
            &policy(),
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .await
        .unwrap();
    assert_eq!(report.verdict, IdentityCompatibilityVerdict::Drift);
    let paper_vid = report
        .indexes
        .iter()
        .find(|item| item.label == "Paper" && item.property == "vid")
        .unwrap();
    assert_eq!(paper_vid.duplicate_groups, 1);
    let paper_arxiv = report
        .indexes
        .iter()
        .find(|item| item.label == "Paper" && item.property == "arxiv_id")
        .unwrap();
    assert_eq!(paper_arxiv.missing_values, 1);
}

#[tokio::test]
async fn malformed_source_digest_and_report_tamper_fail_closed() {
    let store = MockGraphStore::new();
    assert!(
        IdentityCompatibilityUseCase::new(&store)
            .audit(&policy(), "bad")
            .await
            .is_err()
    );

    let mut report = IdentityCompatibilityUseCase::new(&store)
        .audit(
            &policy(),
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        )
        .await
        .unwrap();
    report.import_eligible = !report.import_eligible;
    assert!(report.validate().is_err());
}

#[test]
fn accepted_vid_bytes_are_frozen() {
    assert_eq!(
        da_domain::vid::paper_vid("1206.6423"),
        "258ee2de9f62bbf3bff1353831bee7887271130f8f2108efed348be917076d6a"
    );
    assert_eq!(
        da_domain::vid::author_vid("Alice Smith"),
        "77344f441fd29760d1a85fde01696688bf22be66dfebc02c021725a998ac5594"
    );
    assert_eq!(
        da_domain::vid::reference_vid("Paper A"),
        "87f9f239b8cb85e56172bccba6566fffed568c9583acffbc101b401ac89ba4ca"
    );
    assert_eq!(
        da_domain::vid::paper_vid("arxiv:1206.6423"),
        da_domain::vid::paper_vid("1206.6423")
    );
}
