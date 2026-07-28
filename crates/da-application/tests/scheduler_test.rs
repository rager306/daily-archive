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
    GraphScheduler::add_pending_to(&store, &policy, "2401.00001")
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
}

#[tokio::test]
async fn test_add_pending_idempotent() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00002")
        .await
        .unwrap();
    // Second add should succeed without creating a duplicate
    GraphScheduler::add_pending_to(&store, &policy, "2401.00002")
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
async fn test_load_due_tasks_filters_by_next_retry() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00003")
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];

    // Set next_retry to 0 so the task is immediately due
    store
        .set_node_property_int(node_id, "next_retry", 0)
        .await
        .unwrap();

    let due = GraphScheduler::load_due_tasks_from(&store).await;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].1, "2401.00003");
}

#[tokio::test]
async fn test_record_retry_sets_failed_after_max_retries() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00004")
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    store
        .set_node_property_int(node_id, "next_retry", 0)
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

    GraphScheduler::add_pending_to(&store, &policy, "2401.00005")
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    store
        .set_node_property_int(node_id, "next_retry", 0)
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    GraphScheduler::record_retry_on(&store, &policy, node_id, now)
        .await
        .unwrap();

    let next_retry = store
        .get_node_property_int(node_id, "next_retry")
        .await
        .unwrap_or(-1);
    assert!(
        next_retry > now,
        "retry should be scheduled in the future, got: {next_retry}"
    );
}

#[tokio::test]
async fn test_complete_task_marks_completed() {
    let store = fresh_store();
    let policy = RetryPolicy::default();

    GraphScheduler::add_pending_to(&store, &policy, "2401.00006")
        .await
        .unwrap();
    let nodes = store.get_nodes_by_label("SchedulerTask").await;
    let node_id = nodes[0];
    store
        .set_node_property_int(node_id, "next_retry", 0)
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

    let due_after = GraphScheduler::load_due_tasks_from(&store).await;
    assert!(
        due_after.is_empty(),
        "completed task should not appear in due list"
    );
}
