//! S08/T03 contract tests: measured Java work from a stage tape.
//!
//! Default features, fully offline: no service, no filesystem read beyond the
//! git-tracked fixture and the git-tracked corpus manifest.
//!
//! Negative polarity is the point of this file. An absent, filtered, failed,
//! sectionless, inconsistent, hostile-named or unsafe-pathed tape must each
//! produce a typed refusal or a typed absence — never a plausible-looking zero
//! and never a silent drop.

use da_cli::native_java_work::{
    CoverageClaim, DocumentJavaWork, JAVA_WORK_REDACTION_FORBIDDEN, JavaWorkAggregate,
    JavaWorkCorpus, JavaWorkCorpusDocument, JavaWorkError, JavaWorkInputs, MeasurementState,
    NativeJavaWorkReceipt, NativeJavaWorkReport, NotObservedReason, PartialReason,
    TapeVerification, TapeVerificationState, load_corpus_from_manifest, parse_tape_json,
};
use da_parse::StageTape;
use serde_json::Value;

const FIXTURE: &[u8] = include_bytes!("fixtures/native_java_work_tape.json");
const MANIFEST: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
/// The fixture's recorded dump digest: the identity a verification binds to.
const FIXTURE_DUMP_SHA256: &str =
    "3f07c98096681b085310fd1044e31172229b565701f58ec8894398fdaf2b3326";

fn fixture_value() -> Value {
    serde_json::from_slice(FIXTURE).expect("fixture json")
}

fn tape_from(value: &Value) -> StageTape {
    let bytes = serde_json::to_vec(value).expect("serialize tape json");
    parse_tape_json(&bytes).expect("fixture tape")
}

fn receipt_from(value: &Value) -> Result<NativeJavaWorkReceipt, JavaWorkError> {
    NativeJavaWorkReceipt::from_tape(&tape_from(value))
}

fn corpus() -> JavaWorkCorpus {
    load_corpus_from_manifest(MANIFEST).expect("pinned corpus")
}

fn document(id: &str) -> JavaWorkCorpusDocument {
    corpus()
        .documents
        .into_iter()
        .find(|document| document.document_id == id)
        .expect("pinned document")
}

/// Retarget the fixture's identity onto another pinned corpus document, so a
/// corpus-wide aggregation can be exercised without hardcoding any digest.
fn tape_for(document: &JavaWorkCorpusDocument) -> StageTape {
    let mut value = fixture_value();
    value["source_pdf"] = Value::String(document.source_pdf.clone());
    value["source_sha256"] = Value::String(document.source_sha256.clone());
    value["paper_id"] = Value::String(document.document_id.clone());
    tape_from(&value)
}

fn report(
    tapes: &[StageTape],
    verifications: &[TapeVerification],
    unreadable: usize,
) -> NativeJavaWorkReport {
    NativeJavaWorkReport::assemble(
        &corpus(),
        &JavaWorkInputs {
            tapes,
            verifications,
            unreadable_tape_count: unreadable,
            verification_requested: !verifications.is_empty(),
            verification_gaps: 0,
        },
    )
    .expect("report")
}

fn artifact(report: &NativeJavaWorkReport) -> String {
    String::from_utf8(report.artifact_json_bytes().expect("artifact bytes")).expect("utf8")
}

#[test]
fn fixture_counts_are_typed_sorted_and_bound_to_the_tape_identity() {
    let receipt = receipt_from(&fixture_value()).expect("receipt");
    let counts = receipt.measured().expect("counts");

    assert_eq!(receipt.state(), MeasurementState::Measured);
    assert_eq!(counts.coverage, CoverageClaim::Full);
    assert_eq!(counts.sections, 17);
    assert_eq!(counts.distinct_models, 7);
    assert_eq!(counts.total_invocations, 17);
    assert_eq!(counts.models.len(), 7);
    assert_eq!(
        counts
            .models
            .iter()
            .map(|entry| entry.invocations)
            .sum::<usize>(),
        17
    );
    let mut sorted = counts.models.clone();
    sorted.sort_by(|left, right| left.model.cmp(&right.model));
    assert_eq!(counts.models, sorted, "counts must be ordered");

    assert_eq!(counts.dump_sha256, FIXTURE_DUMP_SHA256);
    assert!(counts.tape_identity.starts_with("sha256:"));
    assert_eq!(counts.tape_verification, TapeVerificationState::NotObserved);
    // The receipt is a repo-relative projection: never the recorder's raw value.
    assert_eq!(counts.source_pdf, document("hub-2507.19457").source_pdf);
}

#[test]
fn occurrence_suffixed_names_are_merged_rather_than_invented() {
    let mut value = fixture_value();
    value["sections"] = Value::from(3);
    value["distinct_models"] = Value::from(1);
    value["models_observed"] = serde_json::json!([
        { "model": "date (occurrence 1 of 2)", "invocations": 2 },
        { "model": "date (occurrence 2 of 2)", "invocations": 1 }
    ]);

    let receipt = receipt_from(&value).expect("receipt");
    let counts = receipt.measured().expect("counts");
    assert_eq!(counts.models.len(), 1);
    assert_eq!(counts.models[0].model, "date");
    assert_eq!(counts.models[0].invocations, 3);
    assert_eq!(counts.total_invocations, 3);
}

#[test]
fn declared_distinct_models_must_match_the_derived_count() {
    let mut value = fixture_value();
    value["sections"] = Value::from(3);
    value["distinct_models"] = Value::from(2);
    value["models_observed"] = serde_json::json!([
        { "model": "date (occurrence 1 of 2)", "invocations": 2 },
        { "model": "date (occurrence 2 of 2)", "invocations": 1 }
    ]);

    assert_eq!(
        receipt_from(&value).unwrap_err(),
        JavaWorkError::DistinctMismatch {
            declared: 2,
            derived: 1
        }
    );
}

#[test]
fn coverage_that_does_not_add_up_is_refused() {
    let mut value = fixture_value();
    value["sections"] = Value::from(99);
    assert_eq!(
        receipt_from(&value).unwrap_err(),
        JavaWorkError::CoverageInconsistent {
            sections: 99,
            counted: 17
        }
    );
}

#[test]
fn failed_uncaptured_and_sectionless_tapes_are_typed_refusals() {
    let mut failed = fixture_value();
    failed["capture_state"] = Value::from("failed");
    failed["failure_code"] = Value::from("timeout");
    failed["failure_phase"] = Value::from("await_response");
    failed["dump_bytes"] = Value::from(0);
    failed["dump_sha256"] = Value::Null;
    failed["sections"] = Value::from(0);
    failed["distinct_models"] = Value::from(0);
    failed["models_observed"] = serde_json::json!([]);
    assert_eq!(
        receipt_from(&failed).unwrap_err(),
        JavaWorkError::CaptureFailed {
            code: "timeout".to_string(),
            phase: "await_response".to_string()
        }
    );

    let mut empty = fixture_value();
    empty["capture_state"] = Value::from("uncaptured_empty");
    empty["dump_bytes"] = Value::from(0);
    empty["dump_sha256"] = Value::Null;
    empty["sections"] = Value::from(0);
    empty["distinct_models"] = Value::from(0);
    empty["models_observed"] = serde_json::json!([]);
    assert_eq!(
        receipt_from(&empty).unwrap_err(),
        JavaWorkError::CaptureStateRefused {
            state: "uncaptured_empty"
        }
    );

    let mut sectionless = fixture_value();
    sectionless["sections"] = Value::from(0);
    sectionless["distinct_models"] = Value::from(0);
    sectionless["models_observed"] = serde_json::json!([]);
    assert_eq!(
        receipt_from(&sectionless).unwrap_err(),
        JavaWorkError::NoSections
    );
}

#[test]
fn a_filtered_tape_is_measured_but_never_claims_full_coverage() {
    let mut value = fixture_value();
    value["models_filter"] = Value::from("segmentation,date");
    value["capture_state"] = Value::from("filtered");

    let receipt = receipt_from(&value).expect("filtered receipt");
    assert_eq!(receipt.state(), MeasurementState::Filtered);
    let counts = receipt.measured().expect("counts");
    assert_eq!(
        counts.coverage,
        CoverageClaim::Partial {
            reason: PartialReason::FilteredTape
        }
    );
    assert_eq!(
        receipt.require_full_coverage().unwrap_err(),
        JavaWorkError::FilteredFullCoverage
    );

    // A filter recorded without the derived state is still a filter.
    let mut only_filter = fixture_value();
    only_filter["models_filter"] = Value::from("date");
    assert_eq!(
        receipt_from(&only_filter).expect("receipt").state(),
        MeasurementState::Filtered
    );
}

#[test]
fn an_absent_measurement_cannot_answer_a_full_coverage_claim() {
    let mut value = fixture_value();
    value["sections"] = Value::from(0);
    value["distinct_models"] = Value::from(0);
    value["models_observed"] = serde_json::json!([]);
    assert_eq!(receipt_from(&value).unwrap_err(), JavaWorkError::NoSections);
    let absent = NativeJavaWorkReceipt::not_observed(NotObservedReason::TapeNotProvided);
    assert_eq!(
        absent.require_full_coverage().unwrap_err(),
        JavaWorkError::NotMeasured
    );
}

#[test]
fn a_tape_without_a_dump_digest_binds_nothing() {
    let mut value = fixture_value();
    value["dump_sha256"] = Value::Null;
    assert_eq!(
        receipt_from(&value).unwrap_err(),
        JavaWorkError::DumpDigestAbsent
    );
}

#[test]
fn unknown_schema_endpoint_and_malformed_identities_are_refused() {
    let mut schema = fixture_value();
    schema["schema_version"] = Value::from("grobid-stage-tape.v2");
    assert_eq!(
        receipt_from(&schema).unwrap_err(),
        JavaWorkError::UnsupportedTapeSchema {
            found: "grobid-stage-tape.v2".to_string()
        }
    );

    let mut endpoint = fixture_value();
    endpoint["endpoint"] = Value::from("banana");
    assert_eq!(
        receipt_from(&endpoint).unwrap_err(),
        JavaWorkError::UnsupportedTapeEndpoint {
            found: "banana".to_string()
        }
    );

    let mut digest = fixture_value();
    digest["source_sha256"] = Value::from("not-a-digest");
    assert_eq!(
        receipt_from(&digest).unwrap_err(),
        JavaWorkError::MalformedIdentity {
            field: "source_sha256"
        }
    );
}

#[test]
fn absolute_traversing_non_unix_and_hostile_names_are_refused() {
    for (path, class) in [
        ("/root/daily-archive/data/x.pdf", "absolute"),
        ("data/../../etc/passwd", "traversing"),
        ("data\\sources\\x.pdf", "non_unix"),
        ("", "empty"),
    ] {
        let mut value = fixture_value();
        value["source_pdf"] = Value::from(path);
        assert_eq!(
            receipt_from(&value).unwrap_err(),
            JavaWorkError::UnsafeSourcePath { path_class: class },
            "path `{path}` must be refused as {class}"
        );
    }

    // A crafted dump header cannot smuggle a path or payload into the artifact:
    // model names are the only dump-derived strings that reach it.
    let mut hostile = fixture_value();
    hostile["models_observed"] = serde_json::json!([
        { "model": "/root/secret.token", "invocations": 17 }
    ]);
    hostile["distinct_models"] = Value::from(1);
    assert_eq!(
        receipt_from(&hostile).unwrap_err(),
        JavaWorkError::UnsafeModelName
    );
}

#[test]
fn a_verification_record_binds_schema_dump_and_release() {
    let tape = tape_from(&fixture_value());
    let bound = TapeVerification {
        tape_schema_version: tape.schema_version.clone(),
        dump_sha256: tape.dump_sha256.clone().expect("digest"),
        backend_version: tape.backend_version.clone(),
        findings: 0,
    };
    let verified =
        NativeJavaWorkReceipt::from_tape_verified(&tape, &bound).expect("verified receipt");
    assert_eq!(
        verified.measured().expect("counts").tape_verification,
        TapeVerificationState::Verified
    );

    let other_tape = TapeVerification {
        dump_sha256: "0".repeat(64),
        ..bound.clone()
    };
    assert_eq!(
        NativeJavaWorkReceipt::from_tape_verified(&tape, &other_tape).unwrap_err(),
        JavaWorkError::VerificationBindingMismatch {
            field: "dump_sha256"
        }
    );

    let other_release = TapeVerification {
        backend_version: "0.0.0".to_string(),
        ..bound.clone()
    };
    assert_eq!(
        NativeJavaWorkReceipt::from_tape_verified(&tape, &other_release).unwrap_err(),
        JavaWorkError::VerificationBindingMismatch {
            field: "backend_version"
        }
    );

    let with_findings = TapeVerification {
        findings: 3,
        ..bound.clone()
    };
    assert_eq!(
        NativeJavaWorkReceipt::from_tape_verified(&tape, &with_findings).unwrap_err(),
        JavaWorkError::VerificationFindings { findings: 3 }
    );
}

#[test]
fn a_run_without_tapes_is_typed_absence_and_carries_no_count_fields() {
    let absent = report(&[], &[], 0);
    assert!(matches!(
        absent.aggregate,
        JavaWorkAggregate::NotObserved {
            reason: NotObservedReason::NoMeasuredDocument
        }
    ));
    for document in &absent.documents {
        match document {
            DocumentJavaWork::NotObserved { reason, .. } => {
                assert_eq!(*reason, NotObservedReason::TapeNotProvided);
            }
            other => panic!("expected typed absence, got {other:?}"),
        }
    }

    let json = artifact(&absent);
    assert!(json.contains("\"measurement_state\": \"not_observed\""));
    assert!(json.contains("\"unit\": \"java_model_invocations\""));
    // No count field exists at all: absence cannot be read as a zero. The unit
    // name legitimately contains the word, so the assertions look for keys.
    assert!(!json.contains("\"total_invocations\":"));
    assert!(!json.contains("\"invocations\":"));
    assert!(!json.contains("\"sections\":"));

    // An unreadable tape is a different typed reason, never a zero count.
    let unreadable = report(&[], &[], 1);
    match &unreadable.documents[0] {
        DocumentJavaWork::NotObserved { reason, .. } => {
            assert_eq!(*reason, NotObservedReason::TapeUnreadable);
        }
        other => panic!("expected typed absence, got {other:?}"),
    }
}

#[test]
fn one_matched_tape_measures_one_document_and_the_aggregate_says_partial() {
    let report = report(&[tape_from(&fixture_value())], &[], 0);

    let hub = report
        .documents
        .iter()
        .find(|document| document.document_id() == "hub-2507.19457")
        .expect("hub document");
    let counts = hub.receipt().expect("receipt").measured().expect("counts");
    assert_eq!(counts.total_invocations, 17);
    assert_eq!(counts.source_pdf, document("hub-2507.19457").source_pdf);

    match &report.aggregate {
        JavaWorkAggregate::Measured(aggregate) => {
            assert_eq!(aggregate.documents_measured, 1);
            assert_eq!(aggregate.documents_refused, 0);
            assert_eq!(aggregate.documents_total, 3);
            assert_eq!(aggregate.total_invocations, 17);
            assert_eq!(
                aggregate.coverage,
                CoverageClaim::Partial {
                    reason: PartialReason::IncompleteCorpus
                }
            );
        }
        other => panic!("expected a measured aggregate, got {other:?}"),
    }

    for document in &report.documents {
        if document.document_id() == "hub-2507.19457" {
            continue;
        }
        match document {
            DocumentJavaWork::NotObserved { reason, .. } => {
                assert_eq!(*reason, NotObservedReason::NoMatchingTape);
            }
            other => panic!("expected typed absence, got {other:?}"),
        }
    }
}

#[test]
fn every_pinned_document_measured_gives_full_coverage_and_summed_totals() {
    let tapes = corpus().documents.iter().map(tape_for).collect::<Vec<_>>();
    let report = report(&tapes, &[], 0);

    match &report.aggregate {
        JavaWorkAggregate::Measured(aggregate) => {
            assert_eq!(aggregate.documents_measured, 3);
            assert_eq!(aggregate.documents_refused, 0);
            assert_eq!(aggregate.coverage, CoverageClaim::Full);
            assert_eq!(aggregate.total_invocations, 51);
            assert_eq!(aggregate.models.len(), 7);
        }
        other => panic!("expected a measured aggregate, got {other:?}"),
    }
    assert!(report.unmatched_tapes.is_empty());
}

#[test]
fn a_tape_outside_the_corpus_is_reported_and_never_dropped() {
    let mut value = fixture_value();
    value["source_pdf"] = Value::from("data/sources/arxiv/xx/0000.00000/source/0000.00000.pdf");
    value["source_sha256"] = Value::from("f".repeat(64));
    value["paper_id"] = Value::from("stray-0000.00000");

    let report = report(&[tape_from(&value)], &[], 0);
    assert_eq!(report.unmatched_tapes, vec!["stray-0000.00000".to_string()]);
    assert!(
        report
            .documents
            .iter()
            .all(|document| matches!(document, DocumentJavaWork::NotObserved { .. }))
    );
    assert!(matches!(
        report.aggregate,
        JavaWorkAggregate::NotObserved {
            reason: NotObservedReason::NoMeasuredDocument
        }
    ));
}

#[test]
fn a_path_matched_tape_with_a_different_digest_is_a_typed_refusal() {
    let mut value = fixture_value();
    // Same recorded path as the pinned hub document, different bytes.
    value["source_sha256"] = Value::from("f".repeat(64));

    let report = report(&[tape_from(&value)], &[], 0);
    let hub = report
        .documents
        .iter()
        .find(|document| document.document_id() == "hub-2507.19457")
        .expect("hub document");
    match hub {
        DocumentJavaWork::Refused { refusal, .. } => {
            assert_eq!(refusal.code, "corpus_digest_mismatch");
        }
        other => panic!("expected a typed refusal, got {other:?}"),
    }
    // The refusal is a refusal, never an aggregate of zero work.
    assert!(matches!(
        report.aggregate,
        JavaWorkAggregate::NotObserved { .. }
    ));
}

#[test]
fn the_artifact_is_byte_deterministic_for_the_same_inputs() {
    let tapes = vec![tape_from(&fixture_value())];
    let first = artifact(&report(&tapes, &[], 0));
    let second = artifact(&report(&tapes, &[], 0));
    assert_eq!(first, second);

    let receipt_a =
        serde_json::to_string(&receipt_from(&fixture_value()).expect("receipt")).expect("json");
    let receipt_b =
        serde_json::to_string(&receipt_from(&fixture_value()).expect("receipt")).expect("json");
    assert_eq!(receipt_a, receipt_b);
}

#[test]
fn the_artifact_carries_no_dump_payload_paths_urls_or_hostnames() {
    let report = report(&[tape_from(&fixture_value())], &[], 0);
    let json = artifact(&report);

    for marker in [
        "=== model:",
        "/root",
        "/home",
        "/tmp",
        "/usr",
        ".wapiti",
        "#mdl#",
        "#rdr#",
        "http://",
        "https://",
        "127.0.0.1",
        "localhost",
        "SENTINEL-NOT-FOR-ARTIFACT",
    ] {
        assert!(!json.contains(marker), "artifact leaked `{marker}`");
    }

    assert!(json.contains("\"h05_closure\": \"not_claimed\""));
    assert!(json.contains("\"java_cpu_wall_clock\": {"));
    assert!(json.contains("\"state\": \"not_observed\""));
    assert!(json.contains("\"deployed\": false"));
    assert!(json.contains("\"import_eligible\": false"));
    assert!(json.contains("\"production_route\": \"forbidden\""));
    assert!(json.contains("\"auto_promote\": \"forbidden\""));
    assert!(json.contains("\"selection\": \"forbidden\""));
    for entry in JAVA_WORK_REDACTION_FORBIDDEN {
        assert!(
            json.contains(entry),
            "the artifact must declare `{entry}` as forbidden"
        );
    }
}
