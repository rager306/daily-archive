//! Integration tests for ExtractionUseCase using mock ports.

#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use da_application::ExtractionUseCase;
use da_domain::entity::EntityType;
use da_ports::extractor::{ExtractResult, ExtractedEntity, Extractor};
use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use da_ports::parser::{ParsedArticle, Section};

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

struct MockGraphStore {
    nodes: Arc<AtomicUsize>,
    props: Mutex<std::collections::HashMap<(u64, String), String>>,
    edges: Mutex<Vec<(u64, u64, String)>>, // (source, target, edge_type)
    // Track node labels for find_node_by_string_property label filtering
    // (MEM495: real SamyamaGraphStore filters by label; mock must match).
    labels: Mutex<std::collections::HashMap<u64, String>>,
}

#[async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Ok(vec![])
    }
    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let id = self.nodes.fetch_add(1, Ordering::SeqCst) as u64;
        self.labels.lock().unwrap().insert(id, label.to_string());
        Ok(id)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.props
            .lock()
            .unwrap()
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_int(
        &self,
        _node_id: u64,
        _key: &str,
        _value: i64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_float(
        &self,
        _node_id: u64,
        _key: &str,
        _value: f64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        _node_id: u64,
        _key: &str,
        _value: bool,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        self.edges
            .lock()
            .unwrap()
            .push((source, target, edge_type.to_string()));
        Ok(0)
    }
    async fn add_vector(
        &self,
        _label: &str,
        _property: &str,
        _node_id: u64,
        _vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _label: &str,
        _property: &str,
        _query: &[f32],
        _k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.nodes.load(Ordering::SeqCst)
    }
    async fn edge_count(&self) -> usize {
        self.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        let props = self.props.lock().unwrap();
        let labels = self.labels.lock().unwrap();
        for ((node_id, k), v) in props.iter() {
            if k == key && v == value {
                // Verify node label matches (real SamyamaGraphStore filters by label).
                if labels.get(node_id).map(|s| s.as_str()) == Some(label) {
                    return Some(*node_id);
                }
            }
        }
        None
    }
    async fn get_incoming_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_node_property_string(&self, _node_id: u64, _key: &str) -> Option<String> {
        None
    }
    async fn get_node_property_int(&self, _node_id: u64, _key: &str) -> Option<i64> {
        None
    }
    async fn get_nodes_by_label(&self, _label: &str) -> Vec<u64> {
        Vec::new()
    }
}

fn make_parsed() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2401.00001".to_string(),
        title: "Test Paper".to_string(),
        abstract_text: "Abstract".to_string(),
        body_text: "body".to_string(),
        sections: vec![Section {
            title: "Method".to_string(),
            text: "We propose TestMethod.".to_string(),
            level: 1,
        }],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        pdf_hash: "hash".to_string(),
    }
}

fn make_store() -> (Arc<AtomicUsize>, MockGraphStore) {
    let nodes = Arc::new(AtomicUsize::new(0));
    let store = MockGraphStore {
        nodes: nodes.clone(),
        props: Mutex::new(std::collections::HashMap::new()),
        edges: Mutex::new(Vec::new()),
        labels: Mutex::new(std::collections::HashMap::new()),
    };
    (nodes, store)
}

/// Helper: create a store with a Paper node pre-populated so that
/// extraction's `find_node_by_string_property("Paper", "arxiv_id", ...)` lookup succeeds.
fn make_store_with_paper(arxiv_id: &str) -> (Arc<AtomicUsize>, MockGraphStore) {
    let (nodes, mut store) = make_store();
    let paper_id = nodes.fetch_add(1, Ordering::SeqCst) as u64;
    store
        .labels
        .lock()
        .unwrap()
        .insert(paper_id, "Paper".to_string());
    store
        .props
        .lock()
        .unwrap()
        .insert((paper_id, "arxiv_id".to_string()), arxiv_id.to_string());
    (nodes, store)
}

#[tokio::test]
async fn test_extraction_writes_entity_nodes() {
    let entities = vec![
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "WMT".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Datasets".to_string(),
            char_start: 0,
            char_end: 3,
            surface: "WMT".to_string(),
        },
    ];
    let (nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.paper_id, "2401.00001");
    assert_eq!(result.entities_extracted, 2);
    assert_eq!(result.graph_node_ids.len(), 2);
    assert_eq!(nodes.load(Ordering::SeqCst), 2); // 2 Entity nodes
    assert!(result.entity_types.contains(&"Method".to_string()));
    assert!(result.entity_types.contains(&"Dataset".to_string()));
}

#[tokio::test]
async fn test_extraction_dedup_same_entity() {
    // Same entity extracted twice → 1 node (idempotent)
    let entities = vec![
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "TestMethod".to_string(),
            entity_type: EntityType::Method,
            section_title: "Method".to_string(),
            char_start: 20,
            char_end: 30,
            surface: "TestMethod".to_string(),
        },
    ];
    let (nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 2);
    assert_eq!(result.graph_node_ids.len(), 2);
    // Only 1 Entity node created (deduped)
    assert_eq!(nodes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_extraction_no_entities() {
    let (nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(
        Box::new(MockExtractor { entities: vec![] }),
        Box::new(store),
    );
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 0);
    assert!(result.graph_node_ids.is_empty());
    assert_eq!(nodes.load(Ordering::SeqCst), 0);
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
            char_start: 0,
            char_end: 10,
            surface: "TestMethod".to_string(),
        },
        ExtractedEntity {
            label: "WMT".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Datasets".to_string(),
            char_start: 0,
            char_end: 3,
            surface: "WMT".to_string(),
        },
    ];
    let (nodes, store) = make_store();
    // Pre-create a Paper node with arxiv_id property
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 2);
    // 1 Paper + 2 Entity = 3 nodes
    assert_eq!(nodes.load(Ordering::SeqCst), 3);
    // 2 MENTIONS edges (Paper → each Entity)
    let edges = use_case.graph_store.edge_count().await;
    assert_eq!(edges, 2, "expected 2 MENTIONS edges, got {edges}");
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
        char_start: 0,
        char_end: 10,
        surface: "TestMethod".to_string(),
    }];
    let (_nodes, store) = make_store();
    // Pre-create a Paper + Section node with title="Methods"
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    let section_node = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(section_node, "title", "Methods".to_string())
        .await
        .unwrap();

    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();
    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.found_in_edges, 1, "expected 1 FOUND_IN edge");
    assert_eq!(result.mentions_edges, 1, "expected 1 MENTIONS edge");
}

#[tokio::test]
async fn test_extraction_no_mentions_when_paper_absent() {
    // If no Paper node exists, entities are created but no MENTIONS edges.
    let entities = vec![ExtractedEntity {
        label: "TestMethod".to_string(),
        entity_type: EntityType::Method,
        section_title: "Method".to_string(),
        char_start: 0,
        char_end: 10,
        surface: "TestMethod".to_string(),
    }];
    let (nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.entities_extracted, 1);
    assert_eq!(nodes.load(Ordering::SeqCst), 1); // 1 Entity, no Paper
    let edges = use_case.graph_store.edge_count().await;
    assert_eq!(edges, 0, "no MENTIONS edges when Paper absent");
}

#[tokio::test]
async fn test_extraction_creates_research_problem_for_improvement_abstract() {
    // Abstract containing "we propose" → ResearchProblem(problem_type="improvement")
    let entities = vec![];
    let (nodes, store) = make_store_with_paper("2401.00001");
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let mut parsed = make_parsed();
    parsed.abstract_text = "We propose a novel method for scaling transformers.".to_string();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.problems_created, 1, "expected 1 ResearchProblem");
    // 1 Paper stub + 1 ResearchProblem (no entities, no sections linked)
    assert!(nodes.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn test_extraction_creates_research_problem_for_explanation_abstract() {
    // Abstract containing "we investigate" → ResearchProblem(problem_type="explanation")
    let entities = vec![];
    let (nodes, store) = make_store_with_paper("2401.00001");
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let mut parsed = make_parsed();
    parsed.abstract_text = "We investigate why neural networks generalize.".to_string();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(result.problems_created, 1);
}

#[tokio::test]
async fn test_extraction_no_research_problem_for_neutral_abstract() {
    // Abstract without trigger phrases → no ResearchProblem
    let entities = vec![];
    let (_nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
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
        char_start: 0,
        char_end: 14,
        surface: "accuracy 0.95".to_string(),
    }];
    let (nodes, store) = make_store_with_paper("2401.00001");
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert_eq!(
        result.observations_created, 1,
        "expected 1 MetricObservation"
    );
    assert!(nodes.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn test_extraction_metric_observation_skipped_without_value() {
    // Metric without nearby number → no MetricObservation
    let entities = vec![ExtractedEntity {
        label: "accuracy".to_string(),
        entity_type: EntityType::Metric,
        section_title: "Results".to_string(),
        char_start: 0,
        char_end: 7,
        surface: "accuracy reported".to_string(),
    }];
    let (_nodes, store) = make_store();
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
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
            char_start: 0,
            char_end: 4,
            surface: "BERT".to_string(),
        },
        ExtractedEntity {
            label: "GLUE".to_string(),
            entity_type: EntityType::Dataset,
            section_title: "Method".to_string(),
            char_start: 10,
            char_end: 14,
            surface: "GLUE".to_string(),
        },
    ];
    let (_nodes, store) = make_store_with_paper("2401.00001");
    let use_case = ExtractionUseCase::new(Box::new(MockExtractor { entities }), Box::new(store));
    let parsed = make_parsed();

    let result = use_case.extract_from_parsed(&parsed).await.unwrap();

    assert!(
        result.evidence_bundles_created >= 1,
        "expected >=1 EvidenceBundle, got {}",
        result.evidence_bundles_created
    );
}
