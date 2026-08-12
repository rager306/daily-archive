use std::path::Path;

use da_adapters::pipeline_ledger::load_pipeline_stage_ledger;
use da_application::pipeline_ledger_audit::{ObservedPipelineStage, audit_pipeline_ledger};

fn observed(id: &str, ordinal: usize) -> ObservedPipelineStage {
    ObservedPipelineStage {
        stage_id: id.into(),
        ordinal,
        status: "complete".into(),
        input_sha256: "a".repeat(64),
        output_sha256: "b".repeat(64),
        context_items_in: 1,
        context_items_out: 1,
        failure_visible: true,
    }
}

fn ledger() -> (Vec<u8>, da_domain::pipeline_ledger::PipelineStageLedger) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/ontology/pipeline_stage_contracts.yaml");
    let bytes = std::fs::read(&path).unwrap();
    let ledger = load_pipeline_stage_ledger(&path).unwrap();
    (bytes, ledger)
}

#[test]
fn canonical_required_path_passes() {
    let (bytes, ledger) = ledger();
    let records = ledger
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| observed(&s.id, i + 1))
        .collect();
    let report = audit_pipeline_ledger(&ledger, &bytes, records);
    assert!(report.pass, "{:?}", report.findings);
    report.validate().unwrap();
    assert!(!report.import_eligible);
}

#[test]
fn missing_duplicate_and_out_of_order_fail_visibly() {
    let (bytes, ledger) = ledger();
    let mut records = ledger
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| observed(&s.id, i + 1))
        .collect::<Vec<_>>();
    records.retain(|record| record.stage_id != "catalog");
    records.push(observed("parse", 99));
    let report = audit_pipeline_ledger(&ledger, &bytes, records);
    assert!(!report.pass);
    assert!(report.findings.iter().any(|x| x.contains("duplicate")));
    assert!(
        report
            .findings
            .iter()
            .any(|x| x.contains("missing required stage catalog"))
    );
    let mut records = ledger
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| observed(&s.id, i + 1))
        .collect::<Vec<_>>();
    records[0].ordinal = 99;
    let report = audit_pipeline_ledger(&ledger, &bytes, records);
    assert!(!report.pass);
    assert!(
        report
            .findings
            .iter()
            .any(|x| x.contains("before dependency acquisition"))
    );
}
