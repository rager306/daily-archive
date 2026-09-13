mod common;

use common::parse_job_contract::{FakeLedger, lifecycle_contract};
use da_domain::ContentDigest;
use da_domain::parse_job::JobSpec;

fn spec() -> JobSpec {
    let mut spec = JobSpec {
        key: ContentDigest::of_text("placeholder"),
        content: vec!["paper-a".into(), "paper-b".into()],
        backend: "grobid".into(),
        projector: "native-v1".into(),
        profile: "structural".into(),
        tuning: vec![("window".into(), "2".into())],
        locations: vec!["relative/source".into()],
    };
    spec.key = spec.identity_key().unwrap();
    spec
}

#[tokio::test]
async fn fake_ledger_contract_covers_idempotency_fences_and_cancel_race() {
    lifecycle_contract(&FakeLedger::default(), spec()).await;
}

#[tokio::test]
async fn invalid_spec_is_rejected_before_state_creation() {
    let ledger = FakeLedger::default();
    let mut invalid = spec();
    invalid.content.clear();
    assert!(matches!(
        da_ports::JobLedger::submit(&ledger, invalid).await,
        Err(da_domain::parse_job::JobLedgerError::InvalidSpec(_))
    ));
}
