use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use da_application::{GraphScheduler, SchedulerRunner, SchedulerTaskDisposition};
use da_domain::scheduler::RetryPolicy;
use da_ports::FixedClock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::{OpenAlexClient, OpenAlexError, OpenAlexResult, OpenAlexWork};

mod common;
use common::mock_graph_store::MockGraphStore;

enum Response {
    Work(Box<OpenAlexWork>),
    Network,
    NotFound,
}

struct FixedOpenAlex(Response);

#[async_trait]
impl OpenAlexClient for FixedOpenAlex {
    async fn fetch_by_arxiv_id(&self, _arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        match &self.0 {
            Response::Work(work) => Ok((**work).clone()),
            Response::Network => Err(OpenAlexError::Network("offline".into())),
            Response::NotFound => Err(OpenAlexError::NotFound("missing".into())),
        }
    }

    async fn search(&self, _query: &str, _limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        Ok(vec![])
    }
}

fn work() -> OpenAlexWork {
    OpenAlexWork {
        id: "https://openalex.org/W123".into(),
        title: "GEPA".into(),
        doi: Some("https://doi.org/10.48550/arxiv.2507.19457".into()),
        publication_date: Some("2025-07-25".into()),
        cited_by_count: 42,
        primary_topic: None,
        topics: vec![],
        concepts: vec![],
        authors: vec![],
        institutions: vec![],
        authorships: vec![],
        referenced_works: vec![],
    }
}

async fn fixture() -> (MockGraphStore, u64, u64) {
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    for (key, value) in [
        ("vid", da_domain::vid::paper_vid("2507.19457")),
        ("arxiv_id", "2507.19457".into()),
        ("title", "GEPA".into()),
    ] {
        store
            .set_node_property_string(paper, key, value)
            .await
            .unwrap();
    }
    store
        .set_node_property_int(paper, "valid_from", 1)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "retrieval_eligible", true)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "import_eligible", false)
        .await
        .unwrap();
    store
        .set_node_property_bool(paper, "openalex_pending", true)
        .await
        .unwrap();
    store
        .set_node_property_int(paper, "schema_version", 1)
        .await
        .unwrap();
    let policy = RetryPolicy::default();
    GraphScheduler::add_pending_to(&store, &policy, "2507.19457", 1)
        .await
        .unwrap();
    let task = store.get_nodes_by_label("SchedulerTask").await[0];
    store
        .set_node_property_int(task, "next_retry", 0)
        .await
        .unwrap();
    (store, paper, task)
}

fn runner(store: &MockGraphStore, response: Response) -> SchedulerRunner {
    let graph: Arc<dyn DirectGraphStore> = Arc::new(store.clone());
    let openalex: Arc<dyn OpenAlexClient> = Arc::new(FixedOpenAlex(response));
    SchedulerRunner::new(graph, openalex).with_clock(Arc::new(FixedClock(
        Utc.with_ymd_and_hms(2026, 8, 8, 12, 0, 0).unwrap(),
    )))
}

#[tokio::test]
async fn successful_enrichment_and_completion_share_one_store() {
    let (store, paper, task) = fixture().await;

    let report = runner(&store, Response::Work(Box::new(work())))
        .run_due()
        .await
        .unwrap();

    assert_eq!(report.completed, 1);
    assert_eq!(report.pending, 0);
    assert_eq!(
        report.outcomes[0].disposition,
        SchedulerTaskDisposition::Completed
    );
    assert_eq!(
        store
            .get_node_property_string(task, "status")
            .await
            .as_deref(),
        Some("completed")
    );
    assert_eq!(
        store
            .get_node_property_string(paper, "openalex_id")
            .await
            .as_deref(),
        Some("https://openalex.org/W123")
    );
    assert_eq!(
        store
            .get_node_property_string(paper, "doi")
            .await
            .as_deref(),
        Some("https://doi.org/10.48550/arxiv.2507.19457")
    );
    assert_eq!(
        store.get_node_property_int(paper, "cited_by_count").await,
        Some(42)
    );
    assert_eq!(
        store
            .snapshot_node(paper)
            .unwrap()
            .get("openalex_pending")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(store.get_nodes_by_label("SchedulerTask").await.len(), 1);
    store.assert_graph_conforms("successful_enrichment_and_completion_share_one_store");
}

#[tokio::test]
async fn successful_openalex_without_paper_never_completes() {
    // Simulate a corrupted scheduler snapshot containing the task but not Paper
    // by constructing an isolated store with the same task lifecycle.
    let isolated = MockGraphStore::new();
    let policy = RetryPolicy::default();
    GraphScheduler::add_pending_to(&isolated, &policy, "2507.19457", 1)
        .await
        .unwrap();
    let isolated_task = isolated.get_nodes_by_label("SchedulerTask").await[0];
    isolated
        .set_node_property_int(isolated_task, "next_retry", 0)
        .await
        .unwrap();

    let report = runner(&isolated, Response::Work(Box::new(work())))
        .run_due()
        .await
        .unwrap();

    assert_eq!(report.completed, 0);
    assert_eq!(report.pending, 1);
    assert_eq!(
        isolated
            .get_node_property_string(isolated_task, "status")
            .await
            .as_deref(),
        Some("pending")
    );
    assert_eq!(isolated.get_nodes_by_label("Paper").await.len(), 0);
}

#[tokio::test]
async fn not_found_advances_one_task_without_creating_duplicate() {
    let (store, _paper, task) = fixture().await;

    let report = runner(&store, Response::NotFound).run_due().await.unwrap();

    assert_eq!(report.completed, 0);
    assert_eq!(report.pending, 1);
    assert_eq!(store.get_nodes_by_label("SchedulerTask").await.len(), 1);
    assert_eq!(
        store.get_node_property_int(task, "retry_count").await,
        Some(1)
    );
}

#[tokio::test]
async fn max_retry_transitions_to_failed_not_completed() {
    let (store, _paper, task) = fixture().await;
    store
        .set_node_property_int(
            task,
            "retry_count",
            RetryPolicy::default().max_retries as i64 - 1,
        )
        .await
        .unwrap();

    let report = runner(&store, Response::Network).run_due().await.unwrap();

    assert_eq!(report.completed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(
        report.outcomes[0].disposition,
        SchedulerTaskDisposition::Failed
    );
    assert_eq!(
        store
            .get_node_property_string(task, "status")
            .await
            .as_deref(),
        Some("failed")
    );
}

#[tokio::test]
async fn adapter_error_advances_retry_and_never_completes() {
    let (store, _paper, task) = fixture().await;

    let report = runner(&store, Response::Network).run_due().await.unwrap();

    assert_eq!(report.completed, 0);
    assert_eq!(report.pending, 1);
    assert_eq!(
        report.outcomes[0].disposition,
        SchedulerTaskDisposition::Pending
    );
    assert_eq!(
        store
            .get_node_property_string(task, "status")
            .await
            .as_deref(),
        Some("pending")
    );
    assert_eq!(
        store.get_node_property_int(task, "retry_count").await,
        Some(1)
    );
    assert!(
        store
            .get_node_property_int(task, "next_retry")
            .await
            .unwrap()
            > 0
    );
}
