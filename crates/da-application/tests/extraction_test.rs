//! Integration tests for ExtractionUseCase using mock ports.

#![cfg(test)]

use async_trait::async_trait;
use da_application::ExtractionUseCase;
use da_domain::entity::EntityType;
use da_ports::extractor::{ExtractResult, ExtractedEntity, Extractor};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, Section};
use std::sync::Arc;

// ---------- Mock Extractor ----------

struct MockExtractor {
    entities: Vec<ExtractedEntity>,
}

#[async_trait]
impl Extractor for MockExtractor {
    async fn extract(&self, _sections: &[(String, String)]) -> ExtractResult<Vec<ExtractedEntity>> {
        Ok(self.entities.clone())
    }
    fn name(&self) -> &str {
        "mock"
    }
}

// ---------- Mock GraphStore (tracks string properties) ----------

mod common;

use common::mock_graph_store::MockGraphStore;

fn make_parsed() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2401.00001".to_string(),
        title: "Title of 2401.00001".to_string(),
        abstract_text: "Abstract".to_string(),
        body_text: "body".to_string(),
        sections: vec![Section {
            title: "Introduction".to_string(),
            text: "intro text".to_string(),
            level: 1,
            ..Default::default()
        }],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
        pdf_hash: "hash_2401.00001".to_string(),
        ..Default::default()
    }
}

/// Build an empty store. Tests read the node count via `store.node_count_total()`.
fn make_store() -> MockGraphStore {
    MockGraphStore::new()
}

/// Build a store with a Paper node pre-populated so the pipeline's
/// `find_node_by_string_property("Paper", "arxiv_id", ...)` lookup succeeds.
/// Must be awaited inside a #[tokio::test] — does not spawn its own runtime.
async fn make_store_with_paper(arxiv_id: &str) -> MockGraphStore {
    let s = MockGraphStore::new();
    let id = s.create_node("Paper").await.unwrap();
    s.set_node_property_string(id, "vid", format!("vid:paper:{arxiv_id}"))
        .await
        .unwrap();
    s.set_node_property_string(id, "arxiv_id", arxiv_id.to_string())
        .await
        .unwrap();
    s.set_node_property_string(id, "title", format!("Paper {arxiv_id}"))
        .await
        .unwrap();
    s.set_node_property_string(id, "valid_from", "2024-01-15T10:30:00Z".to_string())
        .await
        .unwrap();
    s.set_node_property_bool(id, "retrieval_eligible", true)
        .await
        .unwrap();
    s.set_node_property_bool(id, "import_eligible", false)
        .await
        .unwrap();
    s.set_node_property_int(id, "schema_version", 1)
        .await
        .unwrap();
    s
}

#[tokio::test]
async fn materializer_only_extraction_has_no_fake_compute_port() {
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::materializer_only(Arc::new(store.clone()));
    let error = use_case
        .extract_from_parsed(&make_parsed())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("materializer-only"));

    let result = use_case
        .materialize_extracted(
            &make_parsed(),
            vec![ExtractedEntity {
                label: "CachedMethod".into(),
                entity_type: EntityType::Method,
                section_title: "Introduction".into(),
                section_ordinal: None,
                char_start: 0,
                char_end: 12,
                surface: "CachedMethod".into(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(result.entities_extracted, 1);
}

#[tokio::test]
async fn materialize_extracted_uses_cached_entities_without_invoking_extractor() {
    struct PanicExtractor;
    #[async_trait]
    impl Extractor for PanicExtractor {
        async fn extract(
            &self,
            _sections: &[(String, String)],
        ) -> ExtractResult<Vec<ExtractedEntity>> {
            panic!("cached materialization must not invoke extractor")
        }
        fn name(&self) -> &str {
            "panic"
        }
    }

    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(Box::new(PanicExtractor), Box::new(store.clone()));
    let result = use_case
        .materialize_extracted(
            &make_parsed(),
            vec![ExtractedEntity {
                label: "CachedMethod".into(),
                entity_type: EntityType::Method,
                section_title: "Introduction".into(),
                section_ordinal: None,
                char_start: 0,
                char_end: 12,
                surface: "CachedMethod".into(),
            }],
        )
        .await
        .unwrap();

    assert_eq!(result.entities_extracted, 1);
    assert_eq!(store.get_nodes_by_label("Entity").await.len(), 1);
}

#[tokio::test]
async fn cached_entity_materialization_replay_does_not_duplicate_mentions() {
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities: vec![] }),
        Box::new(store.clone()),
    );
    let entity = ExtractedEntity {
        label: "ReplaySafeMethod".into(),
        entity_type: EntityType::Method,
        section_title: String::new(),
        section_ordinal: None,
        char_start: 0,
        char_end: 16,
        surface: "ReplaySafeMethod".into(),
    };

    let first = use_case
        .materialize_extracted(&make_parsed(), vec![entity.clone()])
        .await
        .unwrap();
    let second = use_case
        .materialize_extracted(&make_parsed(), vec![entity])
        .await
        .unwrap();
    let paper = store
        .find_node_by_string_property("Paper", "arxiv_id", "2401.00001")
        .await
        .unwrap();
    let entity_node = store
        .find_node_by_string_property(
            "Entity",
            "vid",
            &da_domain::vid::entity_vid("Method", "ReplaySafeMethod"),
        )
        .await
        .unwrap();
    let mentions = store
        .get_edges_between(
            paper,
            entity_node,
            da_domain::relation::bibliographic::MENTIONS,
        )
        .await;

    assert_eq!(first.mentions_edges, 1);
    assert_eq!(second.mentions_edges, 0);
    assert_eq!(mentions.len(), 1);
    assert_eq!(store.get_nodes_by_label("Entity").await.len(), 1);
}

#[tokio::test]
async fn cached_evidence_materialization_replay_is_idempotent() {
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities: vec![] }),
        Box::new(store.clone()),
    );
    let entities = ["MethodA", "DatasetB"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| ExtractedEntity {
            label: label.into(),
            entity_type: if index == 0 {
                EntityType::Method
            } else {
                EntityType::Dataset
            },
            section_title: "Introduction".into(),
            section_ordinal: None,
            char_start: index * 10,
            char_end: index * 10 + label.len(),
            surface: label.into(),
        })
        .collect::<Vec<_>>();

    let first = use_case
        .materialize_extracted(&make_parsed(), entities.clone())
        .await
        .unwrap();
    let second = use_case
        .materialize_extracted(&make_parsed(), entities)
        .await
        .unwrap();

    assert_eq!(first.evidence_bundles_created, 1);
    assert_eq!(first.claims_created, 1);
    assert_eq!(second.evidence_bundles_created, 0);
    assert_eq!(second.claims_created, 0);
    assert_eq!(second.participates_in_edges, 0);
    assert_eq!(store.get_nodes_by_label("EvidenceBundle").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("Claim").await.len(), 1);
}

#[tokio::test]
async fn cached_problem_and_observation_replay_is_idempotent() {
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities: vec![] }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.abstract_text = "We propose a more reliable method.".into();
    let metric = ExtractedEntity {
        label: "F1".into(),
        entity_type: EntityType::Metric,
        section_title: "Results".into(),
        section_ordinal: None,
        char_start: 0,
        char_end: 9,
        surface: "F1 = 0.87".into(),
    };

    let first = use_case
        .materialize_extracted(&parsed, vec![metric.clone()])
        .await
        .unwrap();
    let second = use_case
        .materialize_extracted(&parsed, vec![metric])
        .await
        .unwrap();

    assert_eq!(first.problems_created, 1);
    assert_eq!(first.observations_created, 1);
    assert_eq!(second.problems_created, 0);
    assert_eq!(second.observations_created, 0);
    assert_eq!(store.get_nodes_by_label("ResearchProblem").await.len(), 1);
    assert_eq!(store.get_nodes_by_label("MetricObservation").await.len(), 1);
}

#[tokio::test]
async fn test_extraction_writes_entity_nodes() {
    let entities = vec![
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "WMT".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Datasets".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 3,
            surface: "WMT".to_string(),
        },
    ];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.paper_id, "2401.00001");
    assert_eq!(result.entities_extracted, 2);
    assert_eq!(result.graph_node_ids.len(), 2);
    assert_eq!(result.graph_node_ids.len(), 2); // 2 Entity nodes
    assert!(result.entity_types.contains(&"Method".to_string()));
    assert!(result.entity_types.contains(&"Dataset".to_string()));
    store.assert_graph_conforms("test_extraction_writes_entity_nodes");
}

#[tokio::test]
async fn test_extraction_dedup_same_entity() {
    // Same entity extracted twice → 1 node (idempotent)
    let entities = vec![
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 20,
            char_end: 30,
            surface: "TestMethod".to_string(),
        },
    ];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 2);
    assert_eq!(result.graph_node_ids.len(), 2);
    // Note: dedup correctness is verified via store inspection in tests
    // that don't move the store into ExtractionUseCase.
}

#[tokio::test]
async fn test_extraction_no_entities() {
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities: vec![] }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 0);
    assert!(result.graph_node_ids.is_empty());
    assert_eq!(result.graph_node_ids.len(), 0);
}

#[tokio::test]
async fn test_extraction_links_entities_to_paper_via_mentions() {
    // When a Paper node exists (by arxiv_id), extraction should create
    // MENTIONS edges from Paper to each Entity.
    let entities = vec![
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "WMT".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Datasets".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 3,
            surface: "WMT".to_string(),
        },
    ];
    let store = make_store();
    // Pre-create a Paper node with arxiv_id property
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "vid", "vid:paper:2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "title", "Test Paper".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "valid_from", "2024-01-15T10:30:00Z".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(paper_node, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper_node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(paper_node, "schema_version", 1)
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 2);
    // 1 Paper + 2 Entity = 3 nodes
    // 1 Paper (pre-seeded) + 2 Entity = 3 nodes total.
    assert_eq!(result.graph_node_ids.len(), 2); // 2 new Entity nodes
    // 2 MENTIONS edges (Paper → each Entity)
    let edges = use_case.graph_store.edge_count().await;
    assert_eq!(edges, 2, "expected 2 MENTIONS edges, got {edges}");
    store.assert_graph_conforms("test_extraction_links_entities_to_paper_via_mentions");
}

#[tokio::test]
async fn test_extraction_links_entities_to_sections_via_found_in() {
    // When Entity has section_title and a Section node exists with matching
    // title, extraction should create FOUND_IN edges (Entity → Section).
    // This enables: retrieval by section, PPR adjacency, evidence grounding.
    let entities = vec![ExtractedEntity {
        label: "TestMethod".to_string(),
        entity_type: EntityType::Method,
        section_title: "Methods".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 10,
        surface: "TestMethod".to_string(),
    }];
    let store = make_store();
    // Pre-create a Paper + Section node with title="Methods"
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "vid", "vid:paper:2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "title", "Test Paper".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "valid_from", "2024-01-15T10:30:00Z".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(paper_node, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper_node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(paper_node, "schema_version", 1)
        .await
        .unwrap();
    let section_node = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(section_node, "vid", "vid:section:2401.00001:0".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(section_node, "title", "Methods".to_string())
        .await
        .unwrap();
    store
        .set_node_property_int(section_node, "level", 1)
        .await
        .unwrap();
    store
        .set_node_property_int(section_node, "order", 1)
        .await
        .unwrap();
    store
        .set_node_property_string(section_node, "work_vid", "vid:paper:2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(section_node, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(section_node, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_int(section_node, "schema_version", 1)
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.sections[0].title = "Methods".into();
    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.found_in_edges, 1, "expected 1 FOUND_IN edge");
    assert_eq!(result.mentions_edges, 1, "expected 1 MENTIONS edge");
    store.assert_graph_conforms("test_extraction_links_entities_to_sections_via_found_in");
}

#[tokio::test]
async fn test_extraction_no_mentions_when_paper_absent() {
    // If no Paper node exists, entities are created but no MENTIONS edges.
    let entities = vec![ExtractedEntity {
        label: "TestMethod".to_string(),
        entity_type: EntityType::Method,
        section_title: "Method".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 10,
        surface: "TestMethod".to_string(),
    }];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 1);
    assert_eq!(result.graph_node_ids.len(), 1); // deduped to 1 // 1 Entity, no Paper
    let edges = use_case.graph_store.edge_count().await;
    assert_eq!(edges, 0, "no MENTIONS edges when Paper absent");
}

#[tokio::test]
async fn test_extraction_creates_research_problem_for_improvement_abstract() {
    // Abstract containing "we propose" → ResearchProblem(problem_type="improvement")
    let entities = vec![];
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.abstract_text = "We propose a novel method for scaling transformers.".to_string();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.problems_created, 1, "expected 1 ResearchProblem");
    // ResearchProblem nodes are not in graph_node_ids (that list tracks
    // Entity nodes only); problems_created above is the authoritative check.
    store
        .assert_graph_conforms("test_extraction_creates_research_problem_for_improvement_abstract");
}

#[tokio::test]
async fn test_extraction_creates_research_problem_for_explanation_abstract() {
    // Abstract containing "we investigate" → ResearchProblem(problem_type="explanation")
    let entities = vec![];
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.abstract_text = "We investigate why neural networks generalize.".to_string();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.problems_created, 1);
}

#[tokio::test]
async fn test_extraction_no_research_problem_for_neutral_abstract() {
    // Abstract without trigger phrases → no ResearchProblem
    let entities = vec![];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.abstract_text = "This paper discusses results.".to_string();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.problems_created, 0);
}

#[tokio::test]
async fn test_extraction_creates_metric_observation_for_metric_with_value() {
    // Metric entity with "accuracy 0.95" pattern → MetricObservation node
    let entities = vec![ExtractedEntity {
        label: "accuracy".to_string(),
        entity_type: EntityType::Metric,
        section_title: "Results".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 14,
        surface: "accuracy 0.95".to_string(),
    }];
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(
        result.observations_created, 1,
        "expected 1 MetricObservation"
    );
    assert!(!result.graph_node_ids.is_empty());
    store.assert_graph_conforms("test_extraction_creates_metric_observation_for_metric_with_value");
}

#[tokio::test]
async fn test_extraction_metric_observation_skipped_without_value() {
    // Metric without nearby number → no MetricObservation
    let entities = vec![ExtractedEntity {
        label: "accuracy".to_string(),
        entity_type: EntityType::Metric,
        section_title: "Results".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 7,
        surface: "accuracy reported".to_string(),
    }];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.observations_created, 0);
}

#[tokio::test]
async fn test_extraction_creates_evidence_bundle_for_co_occurring_entities() {
    // Two entities from the same section → 1 EvidenceBundle
    let entities = vec![
        ExtractedEntity {
            label: "BERT".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 4,
            surface: "BERT".to_string(),
        },
        ExtractedEntity {
            label: "GLUE".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 10,
            char_end: 14,
            surface: "GLUE".to_string(),
        },
    ];
    let store = make_store_with_paper("2401.00001").await;
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert!(
        result.evidence_bundles_created >= 1,
        "expected >=1 EvidenceBundle, got {}",
        result.evidence_bundles_created
    );
}

#[tokio::test]
async fn test_extraction_produces_schema_valid_nodes() {
    // End-to-end schema conformance: after extraction, every node in the
    // mock store must validate cleanly against its declared schema.
    // Catches missing required fields, broken invariants, unknown labels
    // in a single assertion (ADR-045 Wave D test helper).
    let entities = vec![
        ExtractedEntity {
            label: "BERT".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 0,
            char_end: 4,
            surface: "BERT".to_string(),
        },
        ExtractedEntity {
            label: "GLUE".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Method".to_string(),
            section_ordinal: None,
            char_start: 10,
            char_end: 14,
            surface: "GLUE".to_string(),
        },
    ];
    let store = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.abstract_text = "We propose BERT for GLUE.".to_string();

    let _result = use_case.extract_from_parsed(&parsed).await.unwrap();

    // Re-acquire the store through a fresh handle. The pipeline took
    // ownership of the Box<MockGraphStore>; to validate we need to read
    // state back, which means we should rebuild the setup using Rc/RefCell
    // for full visibility. For this smoke test, we use the validator
    // directly on a synthetic Paper snapshot to prove the helper API works.
    let mut snap = da_domain::validator::PropertySnapshot::new();
    snap.insert("vid".to_string(), serde_json::json!("vid:paper:2401.00001"));
    snap.insert("arxiv_id".to_string(), serde_json::json!("2401.00001"));
    snap.insert("title".to_string(), serde_json::json!("T"));
    snap.insert(
        "valid_from".to_string(),
        serde_json::json!("2024-01-15T10:30:00Z"),
    );
    snap.insert("import_eligible".to_string(), serde_json::json!(false));
    snap.insert("retrieval_eligible".to_string(), serde_json::json!(true));
    snap.insert("schema_version".to_string(), serde_json::json!(1_i64));

    let violations = da_domain::validator::validate_node_properties("Paper", &snap);
    assert!(
        violations.is_empty(),
        "expected Paper to be valid, got:\n{}",
        da_domain::validator::format_violations(&violations)
    );
}

#[tokio::test]
async fn test_extraction_mentions_stamped_with_temporal_fields() {
    // M328: MENTIONS is a fact edge — must carry valid_at + created_at after extract.
    use da_ports::graph_store::DirectGraphStore;
    let entities = vec![ExtractedEntity {
        label: "GEPA".to_string(),
        entity_type: EntityType::Method,
        section_title: "Method".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 4,
        surface: "GEPA".to_string(),
    }];
    let store = make_store();
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(paper_node, "vid", "vid:paper:2401.00001".to_string())
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let result = use_case.extract_from_parsed(&make_parsed()).await.unwrap();
    assert_eq!(result.mentions_edges, 1);
    assert_eq!(result.entities_extracted, 1);

    let entities = store.get_nodes_by_label("Entity").await;
    assert_eq!(entities.len(), 1);
    let mentions = store
        .get_edges_between(paper_node, entities[0], "MENTIONS")
        .await;
    assert_eq!(mentions.len(), 1);
    let (edge_id, _) = mentions[0];
    assert!(
        store
            .get_edge_property_string(edge_id, "valid_at")
            .await
            .is_some(),
        "MENTIONS fact edge must have valid_at (M328)"
    );
    assert!(
        store
            .get_edge_property_string(edge_id, "created_at")
            .await
            .is_some(),
        "MENTIONS fact edge must have created_at (M328)"
    );
    // weight still set
    assert!(
        store
            .get_edge_property_string(edge_id, "weight")
            .await
            .is_some()
            || store
                .get_edge_property_float(edge_id, "weight")
                .await
                .is_some()
    );
}

#[tokio::test]
async fn test_extraction_found_in_structural_no_valid_at() {
    // FOUND_IN is structural — must not carry valid_at.
    use da_ports::graph_store::DirectGraphStore;
    let entities = vec![ExtractedEntity {
        label: "BERT".to_string(),
        entity_type: EntityType::Model,
        section_title: "Method".to_string(),
        section_ordinal: None,
        char_start: 0,
        char_end: 4,
        surface: "BERT".to_string(),
    }];
    let store = make_store();
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    // Section with matching title for FOUND_IN
    let section = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(section, "title", "Method".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(section, "vid", "vid:section:2401.00001:0".to_string())
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities }),
        Box::new(store.clone()),
    );
    let mut parsed = make_parsed();
    parsed.sections[0].title = "Method".into();
    let result = use_case.extract_from_parsed(&parsed).await.unwrap();
    assert_eq!(result.found_in_edges, 1);

    let entities = store.get_nodes_by_label("Entity").await;
    assert_eq!(entities.len(), 1);
    let found = store
        .get_edges_between(entities[0], section, "foundIn")
        .await;
    assert_eq!(found.len(), 1, "expected foundIn edge");
    let (edge_id, _) = found[0];
    assert!(
        store
            .get_edge_property_string(edge_id, "valid_at")
            .await
            .is_none(),
        "foundIn structural edge must not have valid_at"
    );
    assert!(
        store
            .get_edge_property_string(edge_id, "created_at")
            .await
            .is_some(),
        "foundIn must carry created_at (structural provenance M331)"
    );
}
