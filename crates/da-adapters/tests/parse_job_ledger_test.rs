use da_adapters::SamyamaJobLedger;
use da_domain::ContentDigest;
use da_domain::parse_job::{AttemptFence, JobLedgerError, JobSpec, JobStatus};
use da_ports::JobLedger;
use tempfile::TempDir;

fn spec(profile: &str) -> JobSpec {
    let mut value = JobSpec {
        key: ContentDigest::of_text("placeholder"),
        content: vec!["paper-a".into(), "paper-b".into()],
        backend: "grobid".into(),
        projector: "native-v1".into(),
        profile: profile.into(),
        tuning: vec![("window".into(), "2".into())],
        locations: vec!["relative/source".into()],
    };
    value.key = value.identity_key().unwrap();
    value
}

#[tokio::test]
async fn durable_ack_reopens_and_events_are_preserved() {
    let dir = TempDir::new().unwrap();
    let ledger = SamyamaJobLedger::open(dir.path()).unwrap();
    let first = ledger.submit(spec("structural")).await.unwrap();
    let claimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    let fence = claimed.attempt.clone().unwrap();
    ledger.checkpoint(&first.job_id, &fence, 7).await.unwrap();
    drop(ledger);

    let reopened = SamyamaJobLedger::open(dir.path()).unwrap();
    let record = reopened.get(&first.job_id).await.unwrap();
    assert_eq!(record.status, JobStatus::Running);
    assert_eq!(record.cursor, 7);
    assert!(reopened.events(&first.job_id).await.unwrap().len() >= 3);
}

#[tokio::test]
async fn idempotency_conflict_and_stale_fence_are_durable() {
    let dir = TempDir::new().unwrap();
    let ledger = SamyamaJobLedger::open(dir.path()).unwrap();
    let first = ledger.submit(spec("structural")).await.unwrap();
    assert_eq!(ledger.submit(spec("structural")).await.unwrap(), first);
    let mut conflict = spec("rich");
    conflict.key = first.identity.key.clone();
    assert!(matches!(
        ledger.submit(conflict).await,
        Err(JobLedgerError::Conflict)
    ));
    let claimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    let mut stale = claimed.attempt.clone().unwrap();
    stale.epoch = stale.epoch.saturating_sub(1);
    assert!(matches!(
        ledger.heartbeat(&first.job_id, &stale).await,
        Err(JobLedgerError::StaleFence { .. })
    ));
    assert!(matches!(
        ledger.claim(&first.job_id, "worker-b").await,
        Err(JobLedgerError::OwnerBusy)
    ));
}

#[tokio::test]
async fn reclaim_fences_stale_worker_and_terminal_state_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let ledger = SamyamaJobLedger::open(dir.path()).unwrap();
    let first = ledger.submit(spec("structural")).await.unwrap();
    let claimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    let old_fence = claimed.attempt.clone().unwrap();
    let reclaimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    let new_fence = reclaimed.attempt.clone().unwrap();

    assert_eq!(reclaimed.status, JobStatus::Running);
    assert!(new_fence.epoch > old_fence.epoch);
    assert!(new_fence.revision > old_fence.revision);
    assert!(matches!(
        ledger.checkpoint(&first.job_id, &old_fence, 1).await,
        Err(JobLedgerError::StaleFence { .. })
    ));

    let checkpointed = ledger
        .checkpoint(&first.job_id, &new_fence, 2)
        .await
        .unwrap();
    assert_eq!(checkpointed.cursor, 2);
    assert!(matches!(
        ledger.checkpoint(&first.job_id, &new_fence, 1).await,
        Err(JobLedgerError::StaleFence { .. })
    ));

    let terminal = ledger
        .complete(
            &first.job_id,
            &checkpointed.attempt.clone().unwrap(),
            ContentDigest::of_text("result-digest"),
        )
        .await
        .unwrap();
    assert_eq!(terminal.status, JobStatus::Succeeded);
    let replay = ledger
        .cancel(&first.job_id)
        .await
        .expect("terminal cancel replay is idempotent");
    assert_eq!(replay, terminal);
    drop(ledger);

    let reopened = SamyamaJobLedger::open(dir.path()).unwrap();
    let recovered = reopened.get(&first.job_id).await.unwrap();
    assert_eq!(recovered, terminal);
    assert!(
        reopened
            .events(&first.job_id)
            .await
            .unwrap()
            .iter()
            .any(|event| event.event == "complete")
    );
}

#[tokio::test]
async fn cancel_wins_and_failure_is_bounded() {
    let dir = TempDir::new().unwrap();
    let ledger = SamyamaJobLedger::open(dir.path()).unwrap();
    let first = ledger.submit(spec("structural")).await.unwrap();
    let claimed = ledger.claim(&first.job_id, "worker-a").await.unwrap();
    let fence = claimed.attempt.clone().unwrap();
    let cancelled = ledger.cancel(&first.job_id).await.unwrap();
    assert_eq!(cancelled.status, JobStatus::CancelRequested);
    assert!(matches!(
        ledger
            .complete(&first.job_id, &fence, ContentDigest::of_text("result"))
            .await,
        Err(JobLedgerError::Cancelled)
    ));

    let second = ledger
        .submit({
            let mut s = spec("another");
            s.content.push("paper-c".into());
            s.key = s.identity_key().unwrap();
            s
        })
        .await
        .unwrap();
    let attempt = ledger
        .claim(&second.job_id, "worker-a")
        .await
        .unwrap()
        .attempt
        .unwrap();
    let oversized = "secret-payload-".repeat(100);
    let failed = ledger
        .fail(&second.job_id, &attempt, oversized)
        .await
        .unwrap();
    assert_eq!(failed.last_error.unwrap().len(), 512);
}

#[allow(dead_code)]
fn _fence_is_explicitly_serializable(fence: AttemptFence) {
    let _ = serde_json::to_string(&fence).unwrap();
}
