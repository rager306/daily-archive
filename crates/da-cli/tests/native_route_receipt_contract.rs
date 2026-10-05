//! S06/T03 (M514-1lss2s) — the route receipt envelope and its independent
//! validator.
//!
//! S04 and S05 proved that a native claim is honest only when it carries a typed
//! envelope **and** an independent validator that refuses the receipt-level
//! mutations a builder cannot express (D505). This file is the S06 half:
//!
//! * the envelope round-trips, refuses unknown fields, and re-serializes to
//!   byte-identical canonical bytes;
//! * its route vocabularies agree *token for token* with the typed aggregate the
//!   route folds (`da_parse::native_route_result`);
//! * its frozen metric dictionary and recomputed composite sha256 revision agree
//!   with the live T02 freeze DATA (`scripts/governor/checks/grobid_absorption_progress.yaml`),
//!   so a ceiling chosen after the numbers cannot be expressed;
//! * the boundary the receipt matrix is checked against resolves from
//!   `data/parse/layout_bakeoff.yaml#java_free_vertical_slice` and
//!   `data/parse/grobid_compatibility.yaml#defaults.cascade_stages` — the sole
//!   sources;
//! * every receipt-level refusal the contract names — `ThresholdClaimed`,
//!   `ProductCoverageClaimed`, `LimitTunedAfterScoring`, `LimitNotFrozen`,
//!   `StaleFreezeRevision`, `LockOpened`, `RedactionLeak`,
//!   `GuardMissingForObservedZero`, `StageOutsideSliceCounted`,
//!   `FamilyOutOfScope`, `BodyGapUntyped`, `PoolingClaimed`, `BindingDrift` —
//!   fires by name on its own mutation, plus the structural refusals.
//!
//! Nothing here reads the gitignored `data/sources/**/*.pdf` corpus, executes a
//! subprocess, touches the network or writes the tree: the pins are recomputed
//! from the committed bytes and every mutation is an in-memory clone.

use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    sha256_hex, sha256_prefixed,
};
use da_cli::native_route_receipt::{
    DEFAULT_ROUTE_ARTIFACT_REL, NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION, ROUTE_BODY_REASON_VOCABULARY,
    ROUTE_BODY_VERDICT_VOCABULARY, ROUTE_CLAIM_WITHOUT_FREEZE_FORBIDDEN,
    ROUTE_DIRECTION_VOCABULARY, ROUTE_FAMILY_DISPOSITION_VOCABULARY, ROUTE_FAMILY_ORDER,
    ROUTE_FLOOR_METRIC_VOCABULARY, ROUTE_FREEZE_SCOPE, ROUTE_FREEZE_STATE_FROZEN,
    ROUTE_FROZEN_METRIC_VOCABULARY, ROUTE_OUT_OF_SLICE_REASON_VOCABULARY,
    ROUTE_REFUSAL_REASON_VOCABULARY, ROUTE_SILENT_CLAMP_FORBIDDEN, ROUTE_STAGE_STANCE_VOCABULARY,
    ROUTE_TUNED_AFTER_SCORING_FORBIDDEN, RouteBodyAxis, RouteBoundaryInput, RouteDeclaredCorpus,
    RouteDeclaredDocument, RouteDocumentIdentity, RouteExpectation, RouteFamilyCell,
    RouteFreezeDeclaration, RouteLimitsBinding, RouteMetricDeclaration, RouteMetricValue,
    RouteObservedDocument, RoutePopulationId, RoutePopulationInput, RouteReceipt,
    RouteReceiptError, RouteStageStanceCell, assert_route_redacted, build_route_receipt,
    validate_route_receipt,
};
use serde_yaml::Value;
use std::collections::BTreeMap;

// ─────────────────────────── committed carriers ───────────────────────────

const GOVERNOR: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const COMPAT: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
const COMPARISON: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");

fn yaml(text: &str) -> Value {
    serde_yaml::from_str(text).expect("the committed carrier parses as YAML")
}

// ─────────────────────────── freeze ───────────────────────────

fn governor_limit_policy() -> Value {
    yaml(GOVERNOR)["native_route_e2e"]["limit_policy"].clone()
}

/// Recompute `sha256(join("<path>:<sha256-hex>", "\n"))` over the four carriers
/// the T02 freeze pins, from their committed bytes.
fn recompute_freeze_revision() -> String {
    let carriers = [
        ("data/parse/layout_bakeoff.yaml", LAYOUT),
        ("data/parse/comparison_contract.yaml", COMPARISON),
        ("data/parse/native_fulltext_composition.yaml", COMPOSITION),
        (
            "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
            PROVENANCE,
        ),
    ];
    let lines: Vec<String> = carriers
        .iter()
        .map(|(path, text)| format!("{}:{}", path, sha256_hex(text.as_bytes())))
        .collect();
    sha256_prefixed(lines.join("\n").as_bytes())
}

fn freeze() -> RouteFreezeDeclaration {
    let policy = governor_limit_policy();
    let freeze_revision = policy["freeze_revision"]["value"]
        .as_str()
        .expect("the freeze revision is a string")
        .to_owned();
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
        freeze_revision,
        metrics,
        direction_vocabulary,
    }
}

// ─────────────────────────── boundary ───────────────────────────

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

fn out_of_slice_reason(stage: &str) -> String {
    yaml(LAYOUT)["java_free_vertical_slice"]["stages_out_of_slice"]
        .as_sequence()
        .expect("stages_out_of_slice is a sequence")
        .iter()
        .find(|entry| entry["stage_id"].as_str() == Some(stage))
        .and_then(|entry| entry["reason"].as_str())
        .expect("every out-of-slice stage names a reason")
        .to_owned()
}

// ─────────────────────────── corpus ───────────────────────────

fn corpus() -> RouteDeclaredCorpus {
    let block = yaml(LAYOUT)["native_route_corpus"].clone();
    let documents = block["documents"]
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
        .collect();
    RouteDeclaredCorpus {
        corpus_id: block["id"].as_str().expect("a corpus id").to_owned(),
        document_count: block["corpus_n"].as_u64().expect("a document count"),
        page_denominator: block["page_denominator"]
            .as_u64()
            .expect("a page denominator"),
        documents,
    }
}

// ─────────────────────────── fixtures ───────────────────────────

fn zero_body() -> RouteBodyAxis {
    RouteBodyAxis {
        plane_preservation: "not_assessable".to_owned(),
        plane_gap_reason: "p1_localization_uncertain".to_owned(),
        plane_d498_reopen_required: false,
        plane_character_material_units: 0,
        plane_absent_units: 0,
        plane_present_in_token_units: 0,
        plane_spanning_token_units: 0,
    }
}

fn stages(boundary: &RouteBoundaryInput) -> Vec<RouteStageStanceCell> {
    boundary
        .cascade_stages
        .iter()
        .map(|stage| {
            if *stage == boundary.admission_stage {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "admission_boundary".to_owned(),
                    reason: None,
                }
            } else if boundary.out_of_slice_stages.contains(stage) {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "out_of_scope".to_owned(),
                    reason: Some(out_of_slice_reason(stage)),
                }
            } else {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "native_measured".to_owned(),
                    reason: None,
                }
            }
        })
        .collect()
}

fn families() -> Vec<RouteFamilyCell> {
    ROUTE_FAMILY_ORDER
        .iter()
        .enumerate()
        .map(|(index, family)| RouteFamilyCell {
            family: (*family).to_owned(),
            disposition: "measured".to_owned(),
            reason: None,
            observed_count: Some(index as u64 + 1),
        })
        .collect()
}

fn observed() -> Vec<RouteObservedDocument> {
    let corpus = corpus();
    let boundary = boundary();
    corpus
        .documents
        .iter()
        .map(|document| RouteObservedDocument {
            id: document.id.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
            stages: stages(&boundary),
            families: families(),
            body: zero_body(),
            route_digest: sha256_prefixed(format!("{}:route", document.id).as_bytes()),
        })
        .collect()
}

fn limits() -> RouteLimitsBinding {
    let freeze = freeze();
    RouteLimitsBinding {
        freeze_revision: freeze.freeze_revision.clone(),
        freeze_state: ROUTE_FREEZE_STATE_FROZEN.to_owned(),
        freeze_owner: "S06".to_owned(),
        freeze_scope: ROUTE_FREEZE_SCOPE.to_owned(),
        tuned_after_scoring: ROUTE_TUNED_AFTER_SCORING_FORBIDDEN.to_owned(),
        zero_is_not_a_measurement: true,
        silent_clamp: ROUTE_SILENT_CLAMP_FORBIDDEN.to_owned(),
        pass_requires_frozen_threshold: true,
        claim_without_freeze: ROUTE_CLAIM_WITHOUT_FREEZE_FORBIDDEN.to_owned(),
        direction_vocabulary: ROUTE_DIRECTION_VOCABULARY
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        metrics: freeze.metrics,
    }
}

fn populations() -> Vec<RoutePopulationInput> {
    let corpus = corpus();
    let identities: Vec<RouteDocumentIdentity> = corpus
        .documents
        .iter()
        .map(|document| RouteDocumentIdentity {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
        })
        .collect();
    let route_metrics = [
        ("native_route_documents", "floor", corpus.document_count),
        ("native_route_pages", "floor", corpus.page_denominator),
        ("native_route_java_calls", "ceiling", 0),
        ("native_route_unmeasured_stages", "ceiling", 96),
        ("native_route_unclassified_families", "ceiling", 0),
    ];
    let metrics = route_metrics
        .iter()
        .map(|(id, direction, value)| RouteMetricValue {
            id: (*id).to_owned(),
            direction: (*direction).to_owned(),
            value: *value,
        })
        .collect();
    vec![RoutePopulationInput {
        population: RoutePopulationId::NativeRoute,
        documents: identities,
        metrics,
    }]
}

fn guard() -> da_cli::native_family_receipt::GuardProvenance {
    da_cli::native_family_receipt::GuardProvenance {
        installed: true,
        identity_isolated: true,
        observed_java_calls: 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    corpus: &RouteDeclaredCorpus,
    boundary: &RouteBoundaryInput,
    freeze: &RouteFreezeDeclaration,
    limits: &RouteLimitsBinding,
    guard: da_cli::native_family_receipt::GuardProvenance,
    observed: &[RouteObservedDocument],
    populations: &[RoutePopulationInput],
) -> Result<RouteReceipt, RouteReceiptError> {
    build_route_receipt(
        &recompute_freeze_revision(),
        corpus,
        boundary,
        freeze,
        limits,
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        guard,
        observed,
        populations,
    )
}

fn receipt() -> RouteReceipt {
    build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed(),
        &populations(),
    )
    .expect("the builder accepts a well-formed receipt")
}

fn expectation() -> RouteExpectation {
    RouteExpectation {
        declaration_revision_sha256: recompute_freeze_revision(),
        corpus: corpus(),
        boundary: boundary(),
        freeze: freeze(),
        required_families: ROUTE_FAMILY_ORDER.iter().map(|f| (*f).to_owned()).collect(),
    }
}

// ─────────────────────────── happy path ───────────────────────────

#[test]
fn the_builder_emits_a_well_formed_receipt_and_the_validator_accepts_it() {
    let receipt = receipt();
    assert_eq!(receipt.schema_version, NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION);
    assert_eq!(receipt.declaration.pointer, "java_free_vertical_slice");
    assert_eq!(receipt.documents.len(), 12, "one row per declared document");
    assert_eq!(receipt.documents[0].stages.len(), cascade_stages().len());
    assert_eq!(
        receipt.documents[0].families.len(),
        ROUTE_FAMILY_ORDER.len()
    );
    assert_eq!(receipt.populations.len(), 1);
    assert_eq!(receipt.populations[0].document_count, 12);
    assert_eq!(receipt.populations[0].page_denominator, 381);
    assert_eq!(receipt.locks.len(), REQUIRED_LOCKS.len());
    assert_eq!(receipt.residuals.unmeasured_stages, 96);
    assert_eq!(receipt.residuals.unclassified_families, 0);
    assert_eq!(
        receipt.residuals.body.plane_preservation, "not_assessable",
        "the S05 residual is carried, not resolved (D499)"
    );
    assert_eq!(receipt.limits.metrics.len(), 15);
    assert_eq!(
        DEFAULT_ROUTE_ARTIFACT_REL,
        "artifacts/parse_compat/native_route_e2e.json"
    );
    validate_route_receipt(&receipt, &expectation()).expect("the receipt validates");
}

#[test]
fn canonical_re_serialization_is_byte_reproducible_and_round_trips() {
    let receipt = receipt();
    let first = receipt.artifact_json_bytes().expect("serializes");
    let parsed = RouteReceipt::parse_json_bytes(&first).expect("parses");
    let second = parsed.artifact_json_bytes().expect("re-serializes");
    assert_eq!(first, second, "canonical bytes must round-trip identically");
    assert_eq!(parsed, receipt);
}

#[test]
fn an_unknown_field_is_refused_by_serde() {
    let bytes = receipt().artifact_json_bytes().expect("serializes");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("parses as json");
    value
        .as_object_mut()
        .expect("the receipt is an object")
        .insert("invented_field".to_owned(), serde_json::Value::Bool(true));
    let mutated = serde_json::to_vec_pretty(&value).expect("reserializes");
    assert!(
        RouteReceipt::parse_json_bytes(&mutated).is_err(),
        "deny_unknown_fields must refuse the extra key"
    );
}

// ─────────────────────────── vocabulary binding ───────────────────────────

#[test]
fn the_route_vocabularies_agree_token_for_token_with_the_typed_aggregate() {
    let theirs: Vec<&str> = da_parse::NATIVE_ROUTE_FAMILY_ORDER
        .iter()
        .map(|family| family.token())
        .collect();
    assert_eq!(ROUTE_FAMILY_ORDER.to_vec(), theirs);
    assert_eq!(
        ROUTE_FAMILY_DISPOSITION_VOCABULARY.to_vec(),
        da_parse::NATIVE_ROUTE_FAMILY_DISPOSITION_VOCABULARY.to_vec()
    );
    assert_eq!(
        ROUTE_STAGE_STANCE_VOCABULARY.to_vec(),
        da_parse::NATIVE_ROUTE_STAGE_STANCE_VOCABULARY.to_vec()
    );
    assert_eq!(
        ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.to_vec(),
        da_parse::NATIVE_ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(
        ROUTE_REFUSAL_REASON_VOCABULARY.to_vec(),
        da_parse::NATIVE_ROUTE_REFUSAL_REASON_VOCABULARY.to_vec()
    );

    let verdicts: Vec<&str> = vec![
        da_parse::NativeRouteBodyVerdict::GranularityOnly.token(),
        da_parse::NativeRouteBodyVerdict::ContentLoss.token(),
        da_parse::NativeRouteBodyVerdict::NotAssessable.token(),
    ];
    assert_eq!(ROUTE_BODY_VERDICT_VOCABULARY.to_vec(), verdicts);

    let gaps: Vec<&str> = vec![
        da_parse::NativeRouteBodyGapReason::Matched.token(),
        da_parse::NativeRouteBodyGapReason::P1NativeTextAbsent { missing_units: 0 }.token(),
        da_parse::NativeRouteBodyGapReason::P1LocalizationUncertain {
            present_in_token_units: 0,
            spanning_token_units: 0,
            character_material_units: 0,
        }
        .token(),
        da_parse::NativeRouteBodyGapReason::P2RegionAssignmentGap {
            units_in_plane_not_in_body: 0,
        }
        .token(),
        da_parse::NativeRouteBodyGapReason::P2LocalizationUncertain {
            present_in_token_units: 0,
            spanning_token_units: 0,
            character_material_units: 0,
        }
        .token(),
        da_parse::NativeRouteBodyGapReason::ScopeMismatch.token(),
        da_parse::NativeRouteBodyGapReason::ScopeJoinMismatch.token(),
        da_parse::NativeRouteBodyGapReason::NormalizationMismatch.token(),
        da_parse::NativeRouteBodyGapReason::OracleEmpty.token(),
        da_parse::NativeRouteBodyGapReason::PopulationIncomplete {
            documents_absent: 0,
        }
        .token(),
    ];
    assert_eq!(ROUTE_BODY_REASON_VOCABULARY.to_vec(), gaps);
}

#[test]
fn the_frozen_limit_dictionary_is_the_live_governor_freeze() {
    let policy = governor_limit_policy();
    let declared = policy["freeze_revision"]["value"]
        .as_str()
        .expect("a declared revision")
        .to_owned();
    assert_eq!(
        recompute_freeze_revision(),
        declared,
        "the composite revision must recompute from the pinned carrier bytes"
    );

    let freeze = freeze();
    let ids: Vec<&str> = freeze.metrics.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        ROUTE_FROZEN_METRIC_VOCABULARY.to_vec(),
        "the module dictionary must be exactly the live freeze"
    );
    for metric in &freeze.metrics {
        let expected = if ROUTE_FLOOR_METRIC_VOCABULARY.contains(&metric.id.as_str()) {
            "floor"
        } else {
            "ceiling"
        };
        assert_eq!(metric.direction, expected, "direction for {}", metric.id);
    }
    assert_eq!(
        freeze.direction_vocabulary,
        ROUTE_DIRECTION_VOCABULARY.to_vec()
    );
    assert_eq!(
        declared,
        receipt().limits.freeze_revision,
        "the emitted receipt binds the recomputed revision"
    );
}

#[test]
fn the_boundary_resolves_from_the_live_data_and_is_total() {
    let boundary = boundary();
    assert_eq!(boundary.cascade_stages.len(), 10, "P0..P9");
    assert_eq!(boundary.admission_stage, "P0");
    assert_eq!(boundary.in_slice_stages, vec!["P1", "P2"]);
    assert_eq!(boundary.out_of_slice_stages.len(), 7);
    for stage in &boundary.out_of_slice_stages {
        assert!(
            ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.contains(&out_of_slice_reason(stage).as_str()),
            "stage {stage} must name a vocabulary reason"
        );
    }
    let union: std::collections::BTreeSet<&str> = boundary
        .in_slice_stages
        .iter()
        .chain(boundary.out_of_slice_stages.iter())
        .map(String::as_str)
        .chain(std::iter::once(boundary.admission_stage.as_str()))
        .collect();
    let cascade: std::collections::BTreeSet<&str> =
        boundary.cascade_stages.iter().map(String::as_str).collect();
    assert_eq!(
        union, cascade,
        "every cascade stage is classified exactly once"
    );
}

// ─────────────────────────── receipt-level refusals ───────────────────────────

#[test]
fn a_raised_threshold_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.thresholds = "accepted".to_owned();
    assert_eq!(
        validate_route_receipt(&receipt, &expectation()),
        Err(RouteReceiptError::ThresholdClaimed {
            found: "accepted".to_owned()
        })
    );
}

#[test]
fn a_product_coverage_claim_is_refused_by_name() {
    let error = build_route_receipt(
        &recompute_freeze_revision(),
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        "native_family",
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("product coverage is not claimable");
    assert_eq!(
        error,
        RouteReceiptError::ProductCoverageClaimed {
            found: "native_family".to_owned()
        }
    );
}

#[test]
fn a_limit_tuned_after_scoring_is_refused_by_name() {
    let mut limits = limits();
    limits.tuned_after_scoring = "allowed".to_owned();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits,
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("a post-scoring tune is refused");
    assert_eq!(
        error,
        RouteReceiptError::LimitTunedAfterScoring {
            found: "allowed".to_owned()
        }
    );
}

#[test]
fn an_unfrozen_limit_is_refused_by_name() {
    let mut limits = limits();
    limits.freeze_state = "provisional".to_owned();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits,
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("an unfrozen limit is refused");
    assert!(matches!(error, RouteReceiptError::LimitNotFrozen { .. }));
}

#[test]
fn a_metric_outside_the_frozen_dictionary_is_refused_by_name() {
    let mut freeze = freeze();
    let mut limits = limits();
    let invented = "invented_metric".to_owned();
    freeze.metrics[0].id = invented.clone();
    limits.metrics[0].id = invented.clone();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze,
        &limits,
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("an invented metric is refused");
    assert_eq!(error, RouteReceiptError::UnknownMetric { id: invented });
}

#[test]
fn a_direction_outside_the_vocabulary_is_refused_by_name() {
    let mut freeze = freeze();
    let mut limits = limits();
    freeze.metrics[0].direction = "sideways".to_owned();
    limits.metrics[0].direction = "sideways".to_owned();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze,
        &limits,
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("an unknown direction is refused");
    assert!(matches!(
        error,
        RouteReceiptError::DirectionOutsideVocabulary { .. }
    ));
}

#[test]
fn a_receipt_with_a_stale_freeze_revision_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.limits.freeze_revision = sha256_prefixed(b"other-freeze");
    let error =
        validate_route_receipt(&receipt, &expectation()).expect_err("a stale revision is refused");
    assert_eq!(
        error,
        RouteReceiptError::StaleFreezeRevision {
            expected: recompute_freeze_revision(),
            found: sha256_prefixed(b"other-freeze"),
        }
    );
}

#[test]
fn an_opened_lock_is_refused_by_name() {
    let mut receipt = receipt();
    receipt
        .locks
        .insert("import_eligible".to_owned(), "true".to_owned());
    let error =
        validate_route_receipt(&receipt, &expectation()).expect_err("an opened lock is refused");
    assert_eq!(
        error,
        RouteReceiptError::LockOpened {
            lock: "import_eligible".to_owned(),
            expected: "false".to_owned(),
            found: "true".to_owned(),
        }
    );
}

#[test]
fn a_leaked_marker_is_refused_by_name() {
    assert_eq!(
        assert_route_redacted("{\"note\":\"http://127.0.0.1:8070\"}"),
        Err(RouteReceiptError::RedactionLeak {
            marker: "http://".to_owned()
        })
    );
    let mut receipt = receipt();
    receipt.limits.freeze_owner = LEAK_MARKERS[0].to_owned();
    let error =
        validate_route_receipt(&receipt, &expectation()).expect_err("a leaked marker is refused");
    assert!(matches!(error, RouteReceiptError::RedactionLeak { .. }));
}

#[test]
fn a_guard_less_zero_is_refused_by_name() {
    let guard = da_cli::native_family_receipt::GuardProvenance {
        installed: false,
        identity_isolated: true,
        observed_java_calls: 0,
    };
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard,
        &observed(),
        &populations(),
    )
    .expect_err("a guard-less zero is refused");
    assert_eq!(
        error,
        RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: 0
        }
    );
}

#[test]
fn a_positive_java_count_is_refused_by_name() {
    let guard = da_cli::native_family_receipt::GuardProvenance {
        installed: true,
        identity_isolated: true,
        observed_java_calls: 3,
    };
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard,
        &observed(),
        &populations(),
    )
    .expect_err("Java on the route is refused");
    assert_eq!(
        error,
        RouteReceiptError::ObservedJavaOnRoute { attempts: 3 }
    );
}

#[test]
fn a_stage_outside_the_slice_counted_as_measured_is_refused_by_name() {
    let mut receipt = receipt();
    let out_of_slice = out_of_slice_reason("P3");
    let cell = receipt.documents[0]
        .stages
        .iter_mut()
        .find(|cell| cell.stage == "P3" && cell.reason.as_deref() == Some(out_of_slice.as_str()))
        .expect("P3 is an out-of-slice cell");
    cell.stance = "native_measured".to_owned();
    cell.reason = None;
    let error = validate_route_receipt(&receipt, &expectation())
        .expect_err("an out-of-slice stage cannot be measured");
    assert_eq!(
        error,
        RouteReceiptError::StageOutsideSliceCounted {
            document: receipt.documents[0].id.clone(),
            stage: "P3".to_owned(),
        }
    );
}

#[test]
fn an_out_of_scope_family_is_refused_by_name() {
    let mut receipt = receipt();
    let document = receipt.documents[0].id.clone();
    receipt.documents[0].families[0].disposition = "out_of_scope".to_owned();
    receipt.documents[0].families[0].reason = None;
    receipt.documents[0].families[0].observed_count = None;
    let error = validate_route_receipt(&receipt, &expectation())
        .expect_err("a required family cannot be out of scope");
    assert_eq!(
        error,
        RouteReceiptError::FamilyOutOfScope {
            document,
            family: ROUTE_FAMILY_ORDER[0].to_owned(),
            token: "out_of_scope".to_owned(),
        }
    );
}

#[test]
fn an_untyped_body_gap_is_refused_by_name() {
    let mut receipt = receipt();
    let document = receipt.documents[0].id.clone();
    receipt.documents[0].body.plane_preservation = "content_loss".to_owned();
    receipt.documents[0].body.plane_d498_reopen_required = false;
    let error = validate_route_receipt(&receipt, &expectation())
        .expect_err("a content-loss body gap must name D498");
    assert_eq!(
        error,
        RouteReceiptError::BodyGapUntyped {
            document,
            detail: "the P1 D498 flag does not match the P1 verdict".to_owned()
        }
    );
}

#[test]
fn a_pooled_population_is_refused_by_name() {
    let mut receipt = receipt();
    let mut pooled = receipt.populations[0].clone();
    pooled.population = RoutePopulationId::Reference;
    receipt.populations.push(pooled);
    let error = validate_route_receipt(&receipt, &expectation())
        .expect_err("populations must stay disjoint");
    assert!(matches!(error, RouteReceiptError::PoolingClaimed { .. }));
}

#[test]
fn a_drifted_binding_is_refused_by_name() {
    let mut receipt = receipt();
    receipt.declaration.revision_sha256 = sha256_prefixed(b"other");
    let error =
        validate_route_receipt(&receipt, &expectation()).expect_err("a drifted binding is refused");
    assert_eq!(
        error,
        RouteReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: recompute_freeze_revision(),
            found: sha256_prefixed(b"other"),
        }
    );
}

// ─────────────────────────── structural refusals ───────────────────────────

#[test]
fn a_missing_stage_cell_is_refused_by_the_builder() {
    let mut observed = observed();
    observed[0].stages.pop();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed,
        &populations(),
    )
    .expect_err("an incomplete stage matrix is refused");
    assert!(matches!(
        error,
        RouteReceiptError::StageMatrixIncomplete { .. }
    ));
}

#[test]
fn a_missing_family_cell_is_refused_by_the_builder() {
    let mut observed = observed();
    observed[0].families.pop();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed,
        &populations(),
    )
    .expect_err("an incomplete family matrix is refused");
    assert!(matches!(
        error,
        RouteReceiptError::FamilyMatrixIncomplete { .. }
    ));
}

#[test]
fn an_unknown_document_is_refused_by_the_builder() {
    let mut observed = observed();
    let mut extra = observed[0].clone();
    extra.id = "ghost-0000.00000".to_owned();
    observed.push(extra);
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed,
        &populations(),
    )
    .expect_err("an undeclared document is refused");
    assert_eq!(
        error,
        RouteReceiptError::UnknownDocument {
            document: "ghost-0000.00000".to_owned()
        }
    );
}

#[test]
fn a_document_sha_drift_is_refused_by_the_builder() {
    let mut observed = observed();
    observed[0].sha256 = "f".repeat(64);
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed,
        &populations(),
    )
    .expect_err("a drifted pin is refused");
    assert!(matches!(error, RouteReceiptError::DocumentShaDrift { .. }));
}

#[test]
fn a_corpus_pin_drift_is_refused_by_the_builder() {
    let mut corpus = corpus();
    corpus.page_denominator = 380;
    let error = build(
        &corpus,
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("a drifted page denominator is refused");
    assert_eq!(
        error,
        RouteReceiptError::CorpusPinDrift {
            expected: 381,
            found: 380
        }
    );
}

#[test]
fn a_missing_population_document_is_refused_by_name() {
    let mut populations = populations();
    populations[0].documents.pop();
    let error = build(
        &corpus(),
        &boundary(),
        &freeze(),
        &limits(),
        guard(),
        &observed(),
        &populations,
    )
    .expect_err("a population must cover the whole corpus");
    assert!(matches!(
        error,
        RouteReceiptError::PopulationStratificationInvalid { .. }
    ));
}

#[test]
fn a_bad_boundary_is_refused_by_name() {
    let mut boundary = boundary();
    boundary.out_of_slice_stages.push("P1".to_owned());
    let error = build(
        &corpus(),
        &boundary,
        &freeze(),
        &limits(),
        guard(),
        &observed(),
        &populations(),
    )
    .expect_err("a doubly classified stage is refused");
    assert!(matches!(error, RouteReceiptError::BoundaryInvalid { .. }));
}
