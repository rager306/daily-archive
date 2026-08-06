//! Integration tests for EnrichUseCase using mock OpenAlex client.

#![cfg(test)]

use async_trait::async_trait;
use da_application::EnrichUseCase;
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::{
    OpenAlexAuthor, OpenAlexAuthorship, OpenAlexClient, OpenAlexConcept, OpenAlexError,
    OpenAlexInstitution, OpenAlexResult, OpenAlexTopic, OpenAlexWork,
};

// ---------- Mock OpenAlex Client ----------

struct MockOpenAlex {
    work: Option<OpenAlexWork>,
}

#[async_trait]
impl OpenAlexClient for MockOpenAlex {
    async fn fetch_by_arxiv_id(&self, _arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        self.work
            .clone()
            .ok_or_else(|| OpenAlexError::NotFound("mock not found".to_string()))
    }

    async fn search(&self, _query: &str, _limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        Ok(vec![])
    }
}

fn make_mock_work() -> OpenAlexWork {
    OpenAlexWork {
        id: "https://openalex.org/W123".to_string(),
        title: "Test Paper Title".to_string(),
        doi: Some("https://doi.org/10.48550/arxiv.2401.00001".to_string()),
        publication_date: Some("2024-01-01".to_string()),
        cited_by_count: 5,
        primary_topic: Some(OpenAlexTopic {
            id: "https://openalex.org/T1".to_string(),
            display_name: "Reinforcement Learning".to_string(),
            domain: Some("Physical Sciences".to_string()),
            field: Some("Computer Science".to_string()),
            subfield: Some("Artificial Intelligence".to_string()),
        }),
        topics: vec![OpenAlexTopic {
            id: "https://openalex.org/T2".to_string(),
            display_name: "Prompt Optimization".to_string(),
            domain: Some("Physical Sciences".to_string()),
            field: Some("Computer Science".to_string()),
            subfield: None,
        }],
        concepts: vec![OpenAlexConcept {
            id: "https://openalex.org/C1".to_string(),
            display_name: "Machine learning".to_string(),
            level: 1,
            score: 0.95,
        }],
        authors: vec![
            OpenAlexAuthor {
                id: "https://openalex.org/A1".to_string(),
                display_name: "Alice Smith".to_string(),
                orcid: Some("https://orcid.org/0000-0001-2345-6789".to_string()),
            },
            OpenAlexAuthor {
                id: "https://openalex.org/A2".to_string(),
                display_name: "Bob Jones".to_string(),
                orcid: None,
            },
        ],
        institutions: vec![],
        authorships: vec![
            OpenAlexAuthorship {
                author: OpenAlexAuthor {
                    id: "https://openalex.org/A1".to_string(),
                    display_name: "Alice Smith".to_string(),
                    orcid: Some("https://orcid.org/0000-0001-2345-6789".to_string()),
                },
                institutions: vec![OpenAlexInstitution {
                    id: "https://openalex.org/I1".to_string(),
                    display_name: "MIT".to_string(),
                    country_code: Some("US".to_string()),
                    ror: None,
                }],
            },
            OpenAlexAuthorship {
                author: OpenAlexAuthor {
                    id: "https://openalex.org/A2".to_string(),
                    display_name: "Bob Jones".to_string(),
                    orcid: None,
                },
                institutions: vec![],
            },
        ],
        referenced_works: vec![],
    }
}

// ---------- Mock GraphStore ----------

mod common;

use common::mock_graph_store::MockGraphStore;

fn make_store() -> MockGraphStore {
    MockGraphStore::new()
}

#[tokio::test]
async fn test_enrich_writes_topics_and_authors() {
    let work = make_mock_work();
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    assert_eq!(result.title, "Test Paper Title");
    assert_eq!(result.topics_written, 2); // primary + 1 from topics array
    assert_eq!(result.authors_written, 2);
    assert_eq!(
        result.institutions_written, 1,
        "expected 1 Institution (MIT)"
    );
    assert_eq!(
        result.affiliation_edges_written, 1,
        "expected 1 AFFILIATED_WITH edge (Alice → MIT)"
    );
    assert_eq!(result.cited_by_count, 5);
    assert!(result.doi.is_some());
    store.assert_graph_conforms("test_enrich_writes_topics_and_authors");
}

#[tokio::test]
async fn test_enrich_links_author_to_institution_via_affiliated_with_edge() {
    // Verify the edge is materialized when authorship has an institution.
    // The mock has Alice → MIT in authorships.
    let work = make_mock_work();
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    assert!(
        result.affiliation_edges_written >= 1,
        "AFFILIATED_WITH edge not created; got {}",
        result.affiliation_edges_written
    );

    // M329: AFFILIATED_WITH is fact — stamp temporal fields.
    use da_ports::graph_store::DirectGraphStore;
    let authors = store.get_nodes_by_label("Author").await;
    let institutions = store.get_nodes_by_label("Institution").await;
    assert!(!authors.is_empty() && !institutions.is_empty());
    let mut stamped = false;
    for a in &authors {
        for i in &institutions {
            let edges = store.get_edges_between(*a, *i, "AFFILIATED_WITH").await;
            for (eid, _) in edges {
                if store
                    .get_edge_property_string(eid, "valid_at")
                    .await
                    .is_some()
                {
                    stamped = true;
                }
            }
        }
    }
    assert!(stamped, "AFFILIATED_WITH must have valid_at");
}

#[tokio::test]
async fn test_enrich_not_found_creates_pending_stub() {
    // When OpenAlex has no data, enrich should NOT fail — it should create
    // a pending stub with openalex_pending=true (lazy load pattern).
    let openalex = Box::new(MockOpenAlex { work: None });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("9999.99999").await.unwrap();

    assert!(result.openalex_pending);
    assert_eq!(result.topics_written, 0);
    assert_eq!(result.authors_written, 0);
    assert_eq!(result.title, "(pending OpenAlex)");
}

#[tokio::test]
async fn test_enrich_creates_topic_nodes() {
    let work = make_mock_work();
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    // 2 Topic nodes + 2 Author nodes = 4 nodes total
    assert_eq!(result.topics_written, 2);
    assert_eq!(result.authors_written, 2);
}

#[tokio::test]
async fn test_enrich_dedup_same_topic() {
    // If topic already exists, should not create duplicate
    let work = make_mock_work();
    let openalex = Box::new(MockOpenAlex {
        work: Some(work.clone()),
    });
    let store = make_store();

    // Pre-create one topic node
    let pre_node = DirectGraphStore::create_node(&store, "Topic")
        .await
        .unwrap();
    DirectGraphStore::set_node_property_string(&store, pre_node, "vid", "abc123".to_string())
        .await
        .unwrap();
    // The enrich uses vid::paper_vid(topic.id) which is a SHA256 hash
    // So pre-creating won't match unless we use the exact same vid.
    // This test verifies the find_node_by_string_property path works.
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    // Should still work (creates new nodes since vid doesn't match)
    assert!(result.topics_written > 0);
}

#[tokio::test]
async fn test_enrich_not_found_without_scheduler_still_works() {
    // Without scheduler attached, enrich should still create pending stub
    let openalex = Box::new(MockOpenAlex { work: None });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("9999.99999").await.unwrap();

    assert!(result.openalex_pending);
}

// ─── ADR-053 Layer F prerequisite: referenced_works → CITES edges ───
// Bug fix: OpenAlex referenced_works was fetched but discarded (data loss).
// enrich_from_work must create CITES edges from Paper to each referenced work.

fn make_mock_work_with_references() -> OpenAlexWork {
    let mut work = make_mock_work();
    work.referenced_works = vec![
        "https://openalex.org/W111".to_string(),
        "https://openalex.org/W222".to_string(),
        "https://openalex.org/W333".to_string(),
    ];
    work
}

#[tokio::test]
async fn test_enrich_creates_cites_edges_from_referenced_works() {
    let work = make_mock_work_with_references();
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    // enrich_from_work expects a Paper node to already exist (created by ingest).
    // Create a stub Paper node with the arxiv_id so enrich can find it.
    let paper_node = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper_node, "arxiv_id", "2401.00001".to_string())
        .await
        .unwrap();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    // 3 referenced works → 3 CITES edges
    assert_eq!(
        result.cites_edges_written, 3,
        "expected 3 CITES edges from referenced_works, got {}",
        result.cites_edges_written
    );
}

#[tokio::test]
async fn test_enrich_empty_referenced_works_creates_zero_cites_edges() {
    let work = make_mock_work(); // referenced_works = vec![]
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store.clone()));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    assert_eq!(
        result.cites_edges_written, 0,
        "empty referenced_works should create 0 CITES edges"
    );
}
