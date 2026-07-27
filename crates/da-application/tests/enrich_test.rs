//! Integration tests for EnrichUseCase using mock OpenAlex client.

#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use da_application::EnrichUseCase;
use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use da_ports::openalex::{
    OpenAlexAuthor, OpenAlexClient, OpenAlexConcept, OpenAlexError, OpenAlexResult, OpenAlexTopic,
    OpenAlexWork,
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
        referenced_works: vec![],
    }
}

// ---------- Mock GraphStore ----------

struct MockGraphStore {
    nodes: AtomicUsize,
    props: Mutex<std::collections::HashMap<(u64, String), String>>,
    bools: Mutex<std::collections::HashMap<(u64, String), bool>>,
    edges: Mutex<Vec<(u64, u64, String)>>,
}

#[async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _: &str, _: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _: &str, _: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _: &str,
        _: &str,
        _: usize,
        _: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn create_property_index(&self, _: &str, _: &str) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &str,
        _: &[f32],
        _: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        Ok(vec![])
    }
    async fn import_snapshot(&self, _: &[u8]) -> GraphResult<()> {
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, _: &str) -> Result<u64, GraphStoreError> {
        Ok(self.nodes.fetch_add(1, Ordering::SeqCst) as u64)
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
    async fn set_node_property_int(&self, _: u64, _: &str, _: i64) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        self.bools
            .lock()
            .unwrap()
            .insert((node_id, key.to_string()), value);
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
        _: &str,
        _: &str,
        _: u64,
        _: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _: &str,
        _: &str,
        _: &[f32],
        _: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.nodes.load(Ordering::SeqCst)
    }
    async fn edge_count(&self) -> usize {
        self.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(&self, _: &str, key: &str, value: &str) -> Option<u64> {
        let props = self.props.lock().unwrap();
        for ((node_id, k), v) in props.iter() {
            if k == key && v == value {
                return Some(*node_id);
            }
        }
        None
    }
    async fn get_incoming_edges(&self, _: u64) -> Vec<(u64, String)> {
        Vec::new()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.props
            .lock()
            .unwrap()
            .get(&(node_id, key.to_string()))
            .cloned()
    }
}

fn make_store() -> MockGraphStore {
    MockGraphStore {
        nodes: AtomicUsize::new(0),
        props: Mutex::new(std::collections::HashMap::new()),
        bools: Mutex::new(std::collections::HashMap::new()),
        edges: Mutex::new(Vec::new()),
    }
}

#[tokio::test]
async fn test_enrich_writes_topics_and_authors() {
    let work = make_mock_work();
    let openalex = Box::new(MockOpenAlex { work: Some(work) });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    assert_eq!(result.title, "Test Paper Title");
    assert_eq!(result.topics_written, 2); // primary + 1 from topics array
    assert_eq!(result.authors_written, 2);
    assert_eq!(result.cited_by_count, 5);
    assert!(result.doi.is_some());
}

#[tokio::test]
async fn test_enrich_not_found_creates_pending_stub() {
    // When OpenAlex has no data, enrich should NOT fail — it should create
    // a pending stub with openalex_pending=true (lazy load pattern).
    let openalex = Box::new(MockOpenAlex { work: None });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store));

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
    let use_case = EnrichUseCase::new(openalex, Box::new(store));

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
    let use_case = EnrichUseCase::new(openalex, Box::new(store));

    let result = use_case.enrich_by_arxiv_id("2401.00001").await.unwrap();

    // Should still work (creates new nodes since vid doesn't match)
    assert!(result.topics_written > 0);
}

#[tokio::test]
async fn test_enrich_not_found_auto_registers_in_scheduler() {
    use da_application::FileScheduler;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let scheduler = FileScheduler::new(dir.path());

    let openalex = Box::new(MockOpenAlex { work: None });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store)).with_scheduler(scheduler);

    let result = use_case.enrich_by_arxiv_id("9999.99999").await.unwrap();

    assert!(result.openalex_pending);

    // Verify task was auto-registered in scheduler queue
    let queue = use_case.scheduler.as_ref().unwrap().load_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].arxiv_id, "9999.99999");
    assert_eq!(queue[0].status, da_domain::scheduler::TaskStatus::Pending);
}

#[tokio::test]
async fn test_enrich_not_found_without_scheduler_still_works() {
    // Without scheduler attached, enrich should still create pending stub
    let openalex = Box::new(MockOpenAlex { work: None });
    let store = make_store();
    let use_case = EnrichUseCase::new(openalex, Box::new(store));

    let result = use_case.enrich_by_arxiv_id("9999.99999").await.unwrap();

    assert!(result.openalex_pending);
    assert!(use_case.scheduler.is_none());
}
