//! Tests for GraphScheduler associate functions (D135).
//!
//! Verifies that load_due_tasks_from / record_retry_on / complete_task_on
//! work against a shared DirectGraphStore without taking ownership.

use da_adapters::SamyamaGraphStore;
use da_application::GraphScheduler;
use da_domain::scheduler::RetryPolicy;
use da_ports::graph_store::DirectGraphStore;

fn fresh_store() -> SamyamaGraphStore {
    SamyamaGraphStore::new()
}

#[tokio::test]
async fn test_add_pending_creates_scheduler_task_with_default_status() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    // Add a pending task
    GraphScheduler::add_pending_to(&store, &policy, "2401.00001", 1_700_000_000)
        .await
        .unwrap();

    // Verify: exactly one SchedulerTask node exists, status=pending
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    assert_eq!(nodes.len(), 1);
    let status = store
        .get_node_property_string(nodes[0], "status")
        .await
        .unwrap_or_default();
    assert_eq!(status, "pending");

    let arxiv_id = store
        .get_node_property_string(nodes[0], "arxiv_id")
        .await
        .unwrap_or_default();
    assert_eq!(arxiv_id, "2401.00001");

    // PIR-008 / M398-S04: SchedulerTask DateTime properties are written as
    // canonical RFC3339 UTC strings derived from the injected clock instant.
    let added_at = store
        .get_node_property_string(nodes[0], "added_at")
        .await
        .expect("added_at must be a canonical RFC3339 string");
    let expected_added_at = chrono::DateTime::from_timestamp(1_700_000_000, 0)
        .unwrap()
        .to_rfc3339();
    assert_eq!(added_at, expected_added_at);
    let next_retry = store
        .get_node_property_string(nodes[0], "next_retry")
        .await
        .expect("next_retry must be a canonical RFC3339 string");
    let next_dt = chrono::DateTime::parse_from_rfc3339(&next_retry)
        .expect("next_retry must parse as RFC3339");
    assert!(
        next_dt.timestamp() >= 1_700_000_000,
        "next_retry must not be in the past, got: {next_retry}"
    );
}

#[tokio::test]
async fn test_add_pending_idempotent() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00002", 1_700_000_000)
        .await
        .unwrap();
    // Second add should succeed without creating a duplicate
    GraphScheduler::add_pending_to(&store, &policy, "2401.00002", 1_700_000_000)
        .await
        .unwrap();

    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    assert_eq!(
        nodes.len(),
        1,
        "duplicate add should not create second task"
    );
}

#[tokio::test]
async fn test_add_pending_reopens_terminal_task_without_duplicate() {
    let store = fresh_store();
    let policy = RetryPolicy::default();
    GraphScheduler::add_pending_to(&store, &policy, "2507.19457", 100)
        .await
        .unwrap();
    let task = store.get_nodes_by_label("SchedulerTask").await[0];
    GraphScheduler::complete_task_on(&store, task, 200)
        .await
        .unwrap();

    GraphScheduler::add_pending_to(&store, &policy, "2507.19457", 300)
        .await
        .unwrap();

    assert_eq!(store.get_nodes_by_label("SchedulerTask").await.len(), 1);
    assert_eq!(
        store
            .get_node_property_string(task, "status")
            .await
            .as_deref(),
        Some("pending")
    );
    assert_eq!(
        store.get_node_property_int(task, "retry_count").await,
        Some(0)
    );
    assert_eq!(
        store.get_node_property_string(task, "vid").await,
        Some(da_domain::scheduler::scheduler_task_vid(
            &da_domain::scheduler::TaskType::OpenAlexEnrich,
            "2507.19457",
        ))
    );
}

#[tokio::test]
async fn test_load_due_tasks_filters_by_next_retry() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00003", 1_700_000_000)
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];

    // PIR-008 / M398-S04: new writes use the canonical RFC3339 UTC string wire
    // form. Rewrite next_retry to the epoch instant so the task is immediately due.
    store
        .set_node_property_string(node_id, "next_retry", "1970-01-01T00:00:00Z".to_string())
        .await
        .unwrap();

    let due = GraphScheduler::load_due_tasks_from(&store, 1_700_000_000).await;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].1, "2401.00003");
}

#[tokio::test]
async fn test_load_due_tasks_reads_legacy_integer_next_retry_snapshot() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00033", 1_700_000_000)
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];

    // PIR-008 / M398-S04: this simulates a pre-PIR-008 snapshot whose SchedulerTask
    // carried an integer Unix-seconds next_retry. read_datetime_secs is the bounded
    // compatibility decoder: legacy integer snapshots stay READABLE, but new writers
    // must emit the canonical RFC3339 UTC string form (validator rejects new ints).
    store
        .set_node_property_int(node_id, "next_retry", 0)
        .await
        .unwrap();

    let due = GraphScheduler::load_due_tasks_from(&store, 1_700_000_000).await;
    assert_eq!(
        due.len(),
        1,
        "legacy integer next_retry must remain readable"
    );
    assert_eq!(due[0].1, "2401.00033");
}

#[tokio::test]
async fn test_record_retry_sets_failed_after_max_retries() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00004", 1_700_000_000)
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    // Canonical RFC3339 UTC string form (epoch instant: immediately due).
    store
        .set_node_property_string(node_id, "next_retry", "1970-01-01T00:00:00Z".to_string())
        .await
        .unwrap();

    // Force the task to be due by setting retry_count high enough to exceed max retries
    store
        .set_node_property_int(node_id, "retry_count", 999)
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    GraphScheduler::record_retry_on(&store, &policy, node_id, now)
        .await
        .unwrap();

    let status = store
        .get_node_property_string(node_id, "status")
        .await
        .unwrap_or_default();
    assert_eq!(status, "failed", "max retries should mark task as failed");
}

#[tokio::test]
async fn test_record_retry_schedules_future_when_not_maxed() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00005", 1_700_000_000)
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    // Canonical RFC3339 UTC string form (epoch instant: immediately due).
    store
        .set_node_property_string(node_id, "next_retry", "1970-01-01T00:00:00Z".to_string())
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    GraphScheduler::record_retry_on(&store, &policy, node_id, now)
        .await
        .unwrap();

    let next_retry = store
        .get_node_property_string(node_id, "next_retry")
        .await
        .expect("next_retry must be a canonical RFC3339 string (PIR-008)");
    let next_dt = chrono::DateTime::parse_from_rfc3339(&next_retry)
        .expect("next_retry must parse as RFC3339")
        .with_timezone(&chrono::Utc);
    let now_dt = chrono::DateTime::from_timestamp(now, 0).unwrap();
    assert!(
        next_dt > now_dt,
        "retry should be scheduled in the future, got: {next_retry}"
    );
}

#[tokio::test]
async fn test_complete_task_marks_completed() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00006", 1_700_000_000)
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    // Canonical RFC3339 UTC string form (epoch instant: immediately due).
    store
        .set_node_property_string(node_id, "next_retry", "1970-01-01T00:00:00Z".to_string())
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    GraphScheduler::complete_task_on(&store, node_id, now)
        .await
        .unwrap();

    let status = store
        .get_node_property_string(node_id, "status")
        .await
        .unwrap_or_default();
    assert_eq!(status, "completed", "task should be marked completed");

    let due_after = GraphScheduler::load_due_tasks_from(&store, 1_700_000_000).await;
    assert!(
        due_after.is_empty(),
        "completed task should not appear in due list"
    );
}

#[tokio::test]
async fn test_add_pending_reparse_to_is_typed_and_idempotent() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_reparse_to(&store, &policy, "2506.07853", 1_700_000_000)
        .await
        .unwrap();
    GraphScheduler::add_pending_reparse_to(&store, &policy, "2506.07853", 1_700_000_000)
        .await
        .unwrap();

    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    assert_eq!(
        nodes.len(),
        1,
        "duplicate reparse add must not create a second task"
    );
    let task_type = store
        .get_node_property_string(nodes[0], "task_type")
        .await
        .unwrap_or_default();
    assert_eq!(task_type, "reparse");
    let arxiv_id = store
        .get_node_property_string(nodes[0], "arxiv_id")
        .await
        .unwrap_or_default();
    assert_eq!(arxiv_id, "2506.07853");
}
