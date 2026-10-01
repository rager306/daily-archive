//! S06/T05 (M514-1lss2s) — contract test for the committed native-route
//! artifact.
//!
//! This test audits the *shape and bindings* of
//! `artifacts/parse_compat/native_route_e2e.json` from committed bytes alone: it
//! never reads the gitignored scholarly PDF corpus. The hermetic byte recompute
//! of the receipt vs its carriers lives in T08; this file proves the artifact is
//! well-formed, total over P0..P9, free of `out_of_scope` required families,
//! bound to the live DATA declaration/corpus/freeze, redacted, and that the
//! independent validator refuses the named receipt-level mutations it owns.

use da_cli::native_family_receipt::{GuardProvenance, REQUIRED_LOCKS, sha256_hex, sha256_prefixed};
use da_cli::native_route_receipt::{
    NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION, ROUTE_DIRECTION_VOCABULARY, ROUTE_FAMILY_ORDER,
    ROUTE_FLOOR_METRIC_VOCABULARY, ROUTE_STAGE_STANCE_VOCABULARY, RouteBoundaryInput,
    RouteDeclaredCorpus, RouteDeclaredDocument, RouteExpectation, RouteFreezeDeclaration,
    RouteMetricDeclaration, RoutePopulationId, RouteReceipt, RouteReceiptError,
    assert_route_redacted, validate_route_receipt,
};
use serde_yaml::Value;
use std::collections::BTreeMap;
const ARTIFACT: &str = include_str!("../../../artifacts/parse_compat/native_route_e2e.json");
const GOVERNOR: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const COMPAT: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
const COMPARISON: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");
const THIS_SOURCE: &str = include_str!("native_route_e2e_contract.rs");

fn receipt() -> RouteReceipt {
    RouteReceipt::parse_json_bytes(ARTIFACT.as_bytes())
        .expect("the committed artifact parses as a RouteReceipt")
}

fn yaml(text: &str) -> Value {
    serde_yaml::from_str(text).expect("the committed carrier parses as YAML")
}

fn governor_limit_policy() -> Value {
    yaml(GOVERNOR)["native_route_e2e"]["limit_policy"].clone()
}

/// Recompute `sha256(join("<path>:<sha256-hex>", "\n"))` over the freeze
/// carriers the T02 freeze declares, from their committed bytes.
fn recompute_freeze_revision() -> String {
    let carriers: BTreeMap<&str, &str> = [
        ("data/parse/layout_bakeoff.yaml", LAYOUT),
        ("data/parse/comparison_contract.yaml", COMPARISON),
        ("data/parse/native_fulltext_composition.yaml", COMPOSITION),
        (
            "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
            PROVENANCE,
        ),
    ]
    .into_iter()
    .collect();
    let declared: Vec<String> = governor_limit_policy()["freeze_revision"]["carriers"]
        .as_sequence()
        .expect("the freeze declares carriers")
        .iter()
        .map(|carrier| {
            let path = carrier["path"].as_str().expect("a carrier path");
            let text = carriers
                .get(path)
                .unwrap_or_else(|| panic!("the test does not carry {path}"));
            format!("{path}:{}", sha256_hex(text.as_bytes()))
        })
        .collect();
    sha256_prefixed(declared.join("\n").as_bytes())
}

fn freeze() -> RouteFreezeDeclaration {
    let policy = governor_limit_policy();
    let route_directions: BTreeMap<String, String> = policy["route_metrics"]
        .as_sequence()
        .expect("route_metrics is a sequence")
        .iter()
        .map(|metric| {
            (
                metric["id"].as_str().expect("a route metric id").to_owned(),
                metric["direction"]
                    .as_str()
                    .expect("a route metric direction")
                    .to_owned(),
            )
        })
        .collect();
    let metrics = policy["metrics"]
        .as_sequence()
        .expect("metrics is a sequence")
        .iter()
        .map(|metric| {
            let id = metric.as_str().expect("a metric id").to_owned();
            let direction = route_directions.get(&id).cloned().unwrap_or_else(|| {
                if ROUTE_FLOOR_METRIC_VOCABULARY.contains(&id.as_str()) {
                    "floor".to_owned()
                } else {
                    "ceiling".to_owned()
                }
            });
            RouteMetricDeclaration { id, direction }
        })
        .collect();
    let direction_vocabulary = policy["direction_vocabulary"]
        .as_sequence()
        .expect("direction_vocabulary is a sequence")
        .iter()
        .map(|value| value.as_str().expect("a direction").to_owned())
        .collect();
    RouteFreezeDeclaration {
        freeze_revision: recompute_freeze_revision(),
        metrics,
        direction_vocabulary,
    }
}

fn cascade_stages() -> Vec<String> {
    yaml(COMPAT)["defaults"]["cascade_stages"]
        .as_mapping()
        .expect("cascade_stages is a mapping")
        .keys()
        .map(|key| key.as_str().expect("a stage id").to_owned())
        .collect()
}

fn boundary() -> RouteBoundaryInput {
    let slice = yaml(LAYOUT)["java_free_vertical_slice"].clone();
    RouteBoundaryInput {
        cascade_stages: cascade_stages(),
        admission_stage: slice["identity_admission_stage"]["stage_id"]
            .as_str()
            .expect("the admission stage id")
            .to_owned(),
        in_slice_stages: slice["stages_in_slice"]
            .as_sequence()
            .expect("stages_in_slice is a sequence")
            .iter()
            .map(|stage| {
                stage["stage_id"]
                    .as_str()
                    .expect("an in-slice stage")
                    .to_owned()
            })
            .collect(),
        out_of_slice_stages: slice["stages_out_of_slice"]
            .as_sequence()
            .expect("stages_out_of_slice is a sequence")
            .iter()
            .map(|stage| {
                stage["stage_id"]
                    .as_str()
                    .expect("an out-of-slice stage")
                    .to_owned()
            })
            .collect(),
    }
}

fn corpus() -> RouteDeclaredCorpus {
    let block = yaml(LAYOUT)["corpus"].clone();
    RouteDeclaredCorpus {
        corpus_id: block["id"].as_str().expect("a corpus id").to_owned(),
        document_count: block["corpus_n"].as_u64().expect("a document count"),
        page_denominator: block["page_denominator"]
            .as_u64()
            .expect("a page denominator"),
        documents: block["documents"]
            .as_sequence()
            .expect("corpus.documents is a sequence")
            .iter()
            .map(|document| RouteDeclaredDocument {
                id: document["id"].as_str().expect("a document id").to_owned(),
                source_pdf: document["source_pdf"]
                    .as_str()
                    .expect("a source path")
                    .to_owned(),
                sha256: document["sha256"]
                    .as_str()
                    .expect("a document sha256")
                    .to_owned(),
                pages: document["pages"].as_u64().expect("a page count"),
            })
            .collect(),
    }
}

fn expectation() -> RouteExpectation {
    RouteExpectation {
        declaration_revision_sha256: sha256_prefixed(LAYOUT.as_bytes()),
        corpus: corpus(),
        boundary: boundary(),
        freeze: freeze(),
        required_families: ROUTE_FAMILY_ORDER.iter().map(|f| (*f).to_owned()).collect(),
    }
}

fn refusal_name(result: Result<(), RouteReceiptError>) -> String {
    let error = result.expect_err("the mutation must be refused");
    format!("{error:?}")
}

fn assert_refused(result: Result<(), RouteReceiptError>, needle: &str) {
    let name = refusal_name(result);
    assert!(
        name.contains(needle),
        "expected a `{needle}` refusal, got `{name}`"
    );
}

// ─────────────────────────────── shape ───────────────────────────────

#[test]
fn the_committed_artifact_parses_is_non_empty_and_is_redacted() {
    assert!(
        !ARTIFACT.trim().is_empty(),
        "the artifact must be non-empty"
    );
    let parsed = receipt();
    assert_eq!(parsed.schema_version, NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION);
    assert_route_redacted(ARTIFACT).expect("the artifact carries no leak marker");
    for marker in ["http://", "https://", "/root", "/home", "/tmp", "<TEI"] {
        assert!(
            !ARTIFACT.contains(marker),
            "the artifact leaked the marker `{marker}`"
        );
    }
}

#[test]
fn the_artifact_is_diagnostic_only_with_the_five_locks_closed() {
    let parsed = receipt();
    assert_eq!(parsed.claim_scope, "diagnostic_only");
    assert_eq!(parsed.product_coverage_effect, "none");
    assert_eq!(parsed.thresholds, "not_accepted");
    assert_eq!(parsed.locks.len(), REQUIRED_LOCKS.len());
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(
            parsed.locks.get(lock).map(String::as_str),
            Some(value),
            "lock `{lock}` is not closed"
        );
    }
}

#[test]
fn the_guard_is_installed_identity_isolated_and_observed_zero() {
    let parsed = receipt();
    assert_eq!(parsed.guard.observer.as_str(), "java_transport_fetch_tei");
    assert!(parsed.guard.installed, "the guard must be installed");
    assert!(
        parsed.guard.identity_isolated,
        "installing the guard must not rotate the identity"
    );
    assert_eq!(
        parsed.guard.observed_java_calls, 0,
        "the route must not consult Java"
    );
}

#[test]
fn the_stage_matrix_is_total_over_p0_to_p9_with_no_unclassified_stage() {
    let parsed = receipt();
    let cascade = cascade_stages();
    assert!(!cascade.is_empty());
    for document in &parsed.documents {
        let seen: Vec<&str> = document
            .stages
            .iter()
            .map(|cell| cell.stage.as_str())
            .collect();
        assert_eq!(
            seen.len(),
            cascade.len(),
            "document `{}` carries {} stage cells for a {}-stage cascade",
            document.id,
            seen.len(),
            cascade.len()
        );
        for stage in &cascade {
            assert!(
                seen.contains(&stage.as_str()),
                "document `{}` does not classify stage `{stage}`",
                document.id
            );
        }
        for cell in &document.stages {
            assert!(
                ROUTE_STAGE_STANCE_VOCABULARY.contains(&cell.stance.as_str()),
                "stage `{}` carries an unclassified stance `{}`",
                cell.stage,
                cell.stance
            );
        }
    }
}

#[test]
fn no_required_family_is_out_of_scope() {
    let parsed = receipt();
    for document in &parsed.documents {
        let families: Vec<&str> = document
            .families
            .iter()
            .map(|c| c.family.as_str())
            .collect();
        assert_eq!(
            families.len(),
            ROUTE_FAMILY_ORDER.len(),
            "document `{}` must carry one cell per declared family",
            document.id
        );
        for family in ROUTE_FAMILY_ORDER {
            assert!(
                families.contains(&family),
                "document `{}` is missing required family `{family}`",
                document.id
            );
        }
        for cell in &document.families {
            assert_ne!(
                cell.disposition, "out_of_scope",
                "required family `{}` is out of scope",
                cell.family
            );
            assert!(
                ["measured", "unmet", "not_observed", "refused"]
                    .contains(&cell.disposition.as_str()),
                "family `{}` carries an unclassified disposition `{}`",
                cell.family,
                cell.disposition
            );
        }
    }
}

#[test]
fn the_population_holds_three_documents_and_the_202_page_denominator() {
    let parsed = receipt();
    assert_eq!(parsed.documents.len(), 3, "the slice pins three documents");
    assert_eq!(parsed.populations.len(), 1);
    let population = &parsed.populations[0];
    assert_eq!(population.population, RoutePopulationId::NativeRoute);
    assert_eq!(population.document_count, 3);
    assert_eq!(population.page_denominator, 202);
    let declared = corpus();
    assert_eq!(declared.page_denominator, 202);
    let pinned: Vec<&str> = declared.documents.iter().map(|d| d.id.as_str()).collect();
    for document in &parsed.documents {
        assert!(
            pinned.contains(&document.id.as_str()),
            "document `{}` is not pinned by the live corpus",
            document.id
        );
    }
}

#[test]
fn the_body_axis_reproduces_the_s05_residual() {
    let parsed = receipt();
    assert_eq!(parsed.residuals.body.plane_preservation, "not_assessable");
    assert_eq!(
        parsed.residuals.body.plane_gap_reason,
        "p1_localization_uncertain"
    );
    assert_eq!(parsed.residuals.body.plane_absent_units, 0);
    assert!(!parsed.residuals.body.plane_d498_reopen_required);
    for document in &parsed.documents {
        assert_eq!(document.body.plane_preservation, "not_assessable");
        assert_eq!(document.body.plane_gap_reason, "p1_localization_uncertain");
    }
}

#[test]
fn the_freeze_revision_is_recomputable_from_the_committed_carriers() {
    let parsed = receipt();
    let policy = governor_limit_policy();
    let declared = policy["freeze_revision"]["value"]
        .as_str()
        .expect("the freeze declares a revision");
    assert_eq!(
        recompute_freeze_revision(),
        declared,
        "the freeze revision must recompute from committed bytes"
    );
    assert_eq!(
        parsed.limits.freeze_revision, declared,
        "the receipt must bind the declared freeze revision"
    );
    assert_eq!(parsed.limits.freeze_state, "frozen");
    assert_eq!(
        parsed.limits.direction_vocabulary,
        ROUTE_DIRECTION_VOCABULARY.to_vec()
    );
    assert_eq!(
        parsed.limits.metrics.len(),
        15,
        "the freeze declares the full measured vocabulary"
    );
}

#[test]
fn canonical_re_serialization_is_byte_reproducible() {
    let parsed = receipt();
    let bytes = parsed
        .artifact_json_bytes()
        .expect("the receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&bytes).expect("canonical bytes are UTF-8"),
        ARTIFACT,
        "the committed artifact must be the canonical re-serialization"
    );
}

#[test]
fn the_independent_validator_accepts_the_committed_bytes() {
    let parsed = receipt();
    validate_route_receipt(&parsed, &expectation())
        .expect("the independent validator accepts the committed artifact");
}

// ─────────────────────── named mutation refusals ───────────────────────

#[test]
fn an_opened_lock_is_refused_by_name() {
    let mut parsed = receipt();
    parsed
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "LockOpened",
    );
}

#[test]
fn a_claimed_threshold_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.thresholds = "accepted".to_owned();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "ThresholdClaimed",
    );
}

#[test]
fn a_product_coverage_claim_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.product_coverage_effect = "coverage".to_owned();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "ProductCoverageClaimed",
    );
}

#[test]
fn a_guard_less_zero_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.guard.installed = false;
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "GuardMissingForObservedZero",
    );
}

#[test]
fn a_positive_java_count_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.guard.observed_java_calls = 1;
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "ObservedJavaOnRoute",
    );
}

#[test]
fn a_stale_freeze_revision_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.limits.freeze_revision = sha256_prefixed(b"drifted");
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "StaleFreezeRevision",
    );
}

#[test]
fn a_drifted_declaration_binding_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.declaration.revision_sha256 = sha256_prefixed(b"drifted");
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "BindingDrift",
    );
}

#[test]
fn a_document_sha_drift_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.documents[0].sha256 = "0".repeat(64);
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "DocumentShaDrift",
    );
}

#[test]
fn an_unknown_document_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.documents[0].id = "unknown-9999.00000".to_owned();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "UnknownDocument",
    );
}

#[test]
fn an_out_of_scope_required_family_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.documents[0].families[0].disposition = "out_of_scope".to_owned();
    parsed.documents[0].families[0].reason = None;
    parsed.documents[0].families[0].observed_count = None;
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "FamilyOutOfScope",
    );
}

#[test]
fn an_out_of_slice_stage_counted_as_measured_is_refused_by_name() {
    let mut parsed = receipt();
    let boundary = boundary();
    let out_of_slice = boundary
        .out_of_slice_stages
        .first()
        .expect("the slice declares an out-of-slice stage")
        .clone();
    let cell = parsed.documents[0]
        .stages
        .iter_mut()
        .find(|cell| cell.stage == out_of_slice)
        .expect("the artifact carries the out-of-slice stage");
    cell.stance = "native_measured".to_owned();
    cell.reason = None;
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "StageOutsideSliceCounted",
    );
}

#[test]
fn a_pooled_population_is_refused_by_name() {
    let mut parsed = receipt();
    let duplicated = parsed.populations[0].documents.clone();
    parsed.populations[0].documents.extend(duplicated);
    parsed.populations[0].document_count = parsed.populations[0].documents.len() as u64;
    parsed.populations[0].page_denominator = parsed.populations[0]
        .documents
        .iter()
        .map(|document| document.pages)
        .sum();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "PoolingClaimed",
    );
}

#[test]
fn an_untyped_body_gap_is_refused_by_name() {
    let mut parsed = receipt();
    parsed.documents[0].body.plane_absent_units = 1;
    parsed.documents[0].body.plane_d498_reopen_required = false;
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "BodyGapUntyped",
    );
}

#[test]
fn a_missing_family_cell_is_refused_by_the_validator() {
    let mut parsed = receipt();
    parsed.documents[0].families.pop();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "FamilyMatrixIncomplete",
    );
}

#[test]
fn a_missing_stage_cell_is_refused_by_the_validator() {
    let mut parsed = receipt();
    parsed.documents[0].stages.pop();
    assert_refused(
        validate_route_receipt(&parsed, &expectation()),
        "StageMatrixIncomplete",
    );
}

#[test]
fn the_guard_provenance_shape_is_the_emit_shape() {
    // The guard row is the same shape the sibling receipts carry, so the
    // governor's guard check binds to one structure rather than three.
    let parsed = receipt();
    let guard = GuardProvenance {
        installed: parsed.guard.installed,
        identity_isolated: parsed.guard.identity_isolated,
        observed_java_calls: parsed.guard.observed_java_calls,
    };
    assert!(guard.installed && guard.identity_isolated);
    assert_eq!(guard.observed_java_calls, 0);
}

// ─────────────────────────── hermeticity guard ───────────────────────────

#[test]
fn the_contract_test_reads_no_gitignored_corpus() {
    // The scholarly PDF corpus is gitignored: an audit that reads it cannot be
    // hermetic. This file is answerable from `include_str!` carriers alone, so
    // the source scan must find no runtime filesystem read at all (the needles
    // are assembled at runtime so the guard cannot match itself).
    let read_call = ["fs::", "read"].concat();
    let corpus_path = ["data/", "sources"].concat();
    assert!(
        !THIS_SOURCE.contains(&read_call),
        "the contract test must not read any filesystem path at run time"
    );
    assert!(
        !THIS_SOURCE.contains(&corpus_path),
        "the contract test must not name the gitignored corpus path"
    );
}
