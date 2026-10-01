//! S06/T08 (M514-1lss2s) — hermetic audit of both S06 route receipts.
//!
//! T03 emits `artifacts/parse_compat/native_route_e2e.json` (the typed native
//! route receipt) and T06 emits `artifacts/parse_compat/native_resource_same_n.json`
//! (the same-n resource comparison). A receipt is a self-declaration until
//! something else re-reads it, so this file is that independent read — the S06
//! counterpart of the S04/S05 hermetic audits. It re-reads the *committed* bytes,
//! recomputes every pin from the committed carriers, and refuses each named
//! mutation without touching the gitignored scholarly PDF corpus.
//!
//! Disciplines proven, each with its own negative:
//!
//! 1. **Replay from the carriers, not from the receipt.** The declaration pin,
//!    the composite freeze revision, the corpus pin and the reference document
//!    set are recomputed from the committed bytes, and the typed receipt must
//!    canonicalize back to the committed artifact byte for byte.
//! 2. **The governor rule and both receipts agree.** The `native_route_e2e`
//!    block is re-read from the check DATA: it must name the same carriers, the
//!    same populations, the same closed vocabularies and the same five locks the
//!    artifacts carry, and its thirteen-label violation vocabulary must be
//!    exactly the label set the committed negative fixture raises.
//! 3. **A counted zero needs a guard.** The zero observed-Java count is evidence
//!    only under an installed, identity-isolated observer; the same receipt
//!    without it is refused. The counter is non-vacuous: a real
//!    `GrobidHttpBackend::fetch_tei` on an unreachable endpoint yields
//!    `Observed(> 0)`, which both receipts refuse.
//! 4. **The route matrix stays total and typed.** Every document carries exactly
//!    one stance per cascade stage P0..P9 and one cell per family, no stage
//!    outside the slice is counted as measured, no required family is spelled
//!    `out_of_scope`, and the reproduced S05 body residual is carried typed
//!    rather than silently dropped.
//! 5. **The resource receipt binds the same freeze and types every absence.**
//!    Both receipts carry the same `freeze_revision` and metric dictionary, and
//!    every unmeasured resource metric is a closed reason, never a number.
//! 6. **Mutations fail by name.** A stale pin, a drifted population, an opened
//!    lock, a threshold claim, a guard-less zero, an out-of-slice stage counted
//!    as measured, an `out_of_scope` family, an untyped body gap, a pooled
//!    population and a metric without an observation state each produce their
//!    own typed refusal.
//!
//! It is gated on `pdf-layout` because the producers it audits ride that lane:
//! the architecture gate executes it in the `da-cli feature-gated example
//! contracts` step, and a default `cargo test --tests` never builds the
//! producers. (This file itself only reads committed bytes.)
//!
//! The audit reads only git-tracked DATA and the committed artifacts; it never
//! opens the gitignored PDF corpus (proved by scanning this source) and never
//! writes a committed artifact — every mutation below is an in-memory clone.
//!
//! Determinism (ADR-051): no network, no clock, no retry. The single transport
//! call uses a malformed `http://` base URL — it fails at URL parsing, so there
//! is no socket, no DNS and no bind/drop race.

#![cfg(feature = "pdf-layout")]

use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS,
    Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, sha256_hex, sha256_prefixed,
};
use da_cli::native_resource_same_n::{
    DEFAULT_SAME_N_ARTIFACT_REL, NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION, SAME_N_ABSENCE_POLICY,
    SAME_N_ABSENCE_REASON_VOCABULARY, SAME_N_FROZEN_CAP_METRICS, SAME_N_MISMATCH_HARD_FAILURE,
    SAME_N_NATIVE_METRIC_ORDER, SAME_N_OBSERVATION_STATE_VOCABULARY, SAME_N_PAIRING_RULE,
    SAME_N_POOLED_FORBIDDEN, SAME_N_POPULATION_VOCABULARY, SAME_N_REFERENCE_METRIC_ORDER,
    SAME_N_REFERENCE_WORK_REL, SAME_N_SAVING_CLAIM_FORBIDDEN, SameNCorpusBinding,
    SameNDocumentIdentity, SameNExpectation, SameNMetricObservation, SameNReceipt,
    SameNReceiptError, validate_same_n_receipt,
};
use da_cli::native_route_receipt::{
    DEFAULT_ROUTE_ARTIFACT_REL, NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION, ROUTE_BODY_REASON_VOCABULARY,
    ROUTE_BODY_VERDICT_VOCABULARY, ROUTE_CORPUS_POINTER, ROUTE_DECLARATION_MANIFEST_REL,
    ROUTE_DECLARATION_POINTER, ROUTE_DIRECTION_VOCABULARY, ROUTE_FAMILY_DISPOSITION_VOCABULARY,
    ROUTE_FAMILY_ORDER, ROUTE_FLOOR_METRIC_VOCABULARY, ROUTE_FROZEN_METRIC_VOCABULARY,
    ROUTE_OUT_OF_SLICE_REASON_VOCABULARY, ROUTE_POPULATION_VOCABULARY,
    ROUTE_REFUSAL_REASON_VOCABULARY, ROUTE_STAGE_STANCE_VOCABULARY, ROUTE_UNMET_REASON_VOCABULARY,
    RouteBoundaryInput, RouteDeclaredCorpus, RouteDeclaredDocument, RouteExpectation,
    RouteFreezeDeclaration, RouteMetricDeclaration, RouteReceipt, RouteReceiptError,
    assert_route_redacted, validate_route_receipt,
};
use da_parse::{
    FailureCode, GrobidHttpBackend, GrobidProfile, JavaInvocationCounter,
    JavaInvocationObservation, ParseBackend, ParseBackendRequest,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ─────────────────────────── committed carriers ───────────────────────────

/// The route receipt under audit, as committed.
const ROUTE_RECEIPT: &str = include_str!("../../../artifacts/parse_compat/native_route_e2e.json");
/// The same-n resource receipt under audit, as committed.
const RESOURCE_RECEIPT: &str =
    include_str!("../../../artifacts/parse_compat/native_resource_same_n.json");
/// The pinned Java work carrier the reference rows bind to.
const WORK: &str = include_str!("../../../artifacts/parse_compat/native_java_work.json");
/// The governor check block that governs both receipts.
const GOVERNOR: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
/// The S03 slice declaration and corpus pin.
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
/// The cascade stage dictionary.
const COMPAT: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
/// The ceiling-dictionary carrier of the T02 freeze.
const COMPARISON: &str = include_str!("../../../data/parse/comparison_contract.yaml");
/// The content carrier of the T02 freeze.
const COMPOSITION: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
/// The oracle-pin carrier of the T02 freeze.
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");
/// The committed negative fixture the governor rule drives.
const NEGATIVE_FIXTURE: &str =
    include_str!("../../../governor/fixtures/absorption_progress/native_route.negative.json");
/// The repository-relative path the governor rule declares for that fixture.
const NEGATIVE_FIXTURE_REL: &str =
    "governor/fixtures/absorption_progress/native_route.negative.json";
/// This file, as committed, for the hermeticity scan.
const THIS_SOURCE: &str = include_str!("native_route_receipt_test.rs");

/// The guard observer both receipts require.
const GUARD_OBSERVER: &str = "java_transport_fetch_tei";

// ─────────────────────────── DATA mirrors ───────────────────────────

#[derive(Debug, Deserialize)]
struct GovDeclaration {
    manifest: String,
    pointer: String,
}

#[derive(Debug, Deserialize)]
struct GovReceipt {
    path: String,
    schema_version: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
struct GovCorpus {
    manifest: String,
    pointer: String,
    document_count: u64,
    page_denominator: u64,
}

#[derive(Debug, Deserialize)]
struct GovPairingVocabulary {
    rule: String,
    pooled: String,
    saving_claim: String,
    mismatch_disposition: String,
}

#[derive(Debug, Deserialize)]
struct GovExpected {
    claim_scope: String,
    product_coverage_effect: String,
    thresholds: String,
    guard_observer: String,
    locks: BTreeMap<String, serde_yaml::Value>,
    population_vocabulary: Vec<String>,
    native_population: String,
    resource_population_vocabulary: Vec<String>,
    observation_state_vocabulary: Vec<String>,
    absence_reason_vocabulary: Vec<String>,
    native_metric_vocabulary: Vec<String>,
    reference_metric_vocabulary: Vec<String>,
    reference_source_kind: String,
    pairing_vocabulary: GovPairingVocabulary,
    body_verdict_vocabulary: Vec<String>,
    body_reason_vocabulary: Vec<String>,
    stage_stance_vocabulary: Vec<String>,
    out_of_slice_stance: String,
    admission_stance: String,
    family_disposition_vocabulary: Vec<String>,
    family_unmet_reason_vocabulary: Vec<String>,
    family_refusal_reason_vocabulary: Vec<String>,
    forbidden_family_token: String,
    residual_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GovNegativeExpect {
    require_violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GovNegative {
    id: String,
    path: String,
    expect: GovNegativeExpect,
}

#[derive(Debug, Deserialize)]
struct GovFixtures {
    negative: GovNegative,
}

#[derive(Debug, Deserialize)]
struct GovRule {
    declaration: GovDeclaration,
    receipt: GovReceipt,
    resource_receipt: GovReceipt,
    corpus: GovCorpus,
    limit_policy: serde_yaml::Value,
    expected: GovExpected,
    violations: Vec<String>,
    fixtures: GovFixtures,
}

#[derive(Debug, Deserialize)]
struct GovCheck {
    native_route_e2e: GovRule,
}

#[derive(Debug, Deserialize)]
struct NegativeEntry {
    violation: String,
}

// ─────────────────────────── helpers ───────────────────────────

fn route() -> RouteReceipt {
    RouteReceipt::parse_json_bytes(ROUTE_RECEIPT.as_bytes())
        .expect("the committed route receipt must parse")
}

fn same_n() -> SameNReceipt {
    SameNReceipt::parse_json_bytes(RESOURCE_RECEIPT.as_bytes())
        .expect("the committed resource receipt must parse")
}

fn yaml(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).expect("the committed carrier parses as YAML")
}

fn gov() -> GovRule {
    serde_yaml::from_str::<GovCheck>(GOVERNOR)
        .expect("the committed governor check parses")
        .native_route_e2e
}

fn governor_limit_policy() -> serde_yaml::Value {
    gov().limit_policy
}

/// Recompute `sha256(join("<path>:<sha256-hex>", "\n"))` over the freeze
/// carriers the T02 freeze declares, from their committed bytes and in the
/// declared order.
fn recompute_freeze_revision() -> String {
    let policy = governor_limit_policy();
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
    let declared: Vec<String> = policy["freeze_revision"]["carriers"]
        .as_sequence()
        .expect("the freeze declares carriers")
        .iter()
        .map(|carrier| carrier["path"].as_str().expect("a carrier path").to_owned())
        .collect();
    let lines: Vec<String> = declared
        .iter()
        .map(|path| {
            let bytes = carriers
                .get(path.as_str())
                .unwrap_or_else(|| panic!("carrier `{path}` is not committed"));
            format!("{path}:{}", sha256_hex(bytes.as_bytes()))
        })
        .collect();
    sha256_prefixed(lines.join("\n").as_bytes())
}

/// The T02 freeze, rebuilt from the governor limit policy.
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

/// Every cascade stage id, in cascade order, from the stage carrier.
fn cascade_stages() -> Vec<String> {
    yaml(COMPAT)["defaults"]["cascade_stages"]
        .as_mapping()
        .expect("cascade_stages is a mapping")
        .keys()
        .map(|key| key.as_str().expect("a stage id").to_owned())
        .collect()
}

/// The declared slice boundary, resolved by pointer.
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

/// The typed reason the declaration gives an out-of-slice stage.
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

/// The declared corpus, rebuilt from the committed declaration bytes.
fn declared_route_corpus() -> RouteDeclaredCorpus {
    let block = yaml(LAYOUT)["corpus"].clone();
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

/// The same-n corpus binding, rebuilt from the committed declaration bytes.
fn declared_same_n_corpus() -> SameNCorpusBinding {
    let block = yaml(LAYOUT)["corpus"].clone();
    let documents = block["documents"]
        .as_sequence()
        .expect("corpus.documents is a sequence")
        .iter()
        .map(|document| SameNDocumentIdentity {
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
    SameNCorpusBinding {
        document_count: block["corpus_n"].as_u64().expect("a document count"),
        page_denominator: block["page_denominator"]
            .as_u64()
            .expect("a page denominator"),
        documents,
    }
}

/// The reference document set the pinned work carrier declares.
fn reference_documents() -> Vec<SameNDocumentIdentity> {
    let work = yaml(WORK);
    work["corpus"]["documents"]
        .as_sequence()
        .expect("work corpus documents")
        .iter()
        .map(|entry| SameNDocumentIdentity {
            id: entry["id"].as_str().expect("an id").to_owned(),
            source_pdf: entry["source_pdf"]
                .as_str()
                .expect("a source path")
                .to_owned(),
            sha256: entry["sha256"].as_str().expect("a sha256").to_owned(),
            pages: entry["pages"].as_u64().expect("a page count"),
        })
        .collect()
}

fn route_expectation() -> RouteExpectation {
    RouteExpectation {
        declaration_revision_sha256: sha256_prefixed(LAYOUT.as_bytes()),
        corpus: declared_route_corpus(),
        boundary: boundary(),
        freeze: freeze(),
        required_families: ROUTE_FAMILY_ORDER.iter().map(|f| (*f).to_owned()).collect(),
    }
}

fn same_n_expectation() -> SameNExpectation {
    SameNExpectation {
        freeze_revision: recompute_freeze_revision(),
        corpus: declared_same_n_corpus(),
        reference_documents: reference_documents(),
    }
}

/// A YAML scalar lock value as the string token the artifact carries.
fn lock_token(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::Bool(flag) => flag.to_string(),
        serde_yaml::Value::String(text) => text.clone(),
        serde_yaml::Value::Number(number) => number.to_string(),
        other => panic!("a lock value must be a scalar, found {other:?}"),
    }
}

fn required_locks_map() -> BTreeMap<String, String> {
    REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect()
}

/// The gitignored corpus root, assembled at runtime so this source never spells
/// the path out (which is what the hermeticity scan below proves).
fn corpus_root_marker() -> String {
    ["data", "sources"].join("/")
}

/// Every metric row of the resource receipt, across populations and documents.
fn same_n_rows(receipt: &SameNReceipt) -> Vec<&SameNMetricObservation> {
    let mut rows: Vec<&SameNMetricObservation> = Vec::new();
    for population in &receipt.populations {
        rows.extend(population.metrics.iter());
    }
    for document in &receipt.documents {
        rows.extend(document.reference.iter());
    }
    rows
}

/// Recompute the resource closure counters from the typed rows.
fn recompute_same_n_residuals(receipt: &SameNReceipt) -> (u64, u64, u64, u64) {
    let mut observed = 0u64;
    let mut absent = 0u64;
    let mut untyped = 0u64;
    let mut bound = 0u64;
    for row in same_n_rows(receipt) {
        match row.state.as_str() {
            "observed" => {
                observed += 1;
                if row.source.as_ref().map(|source| source.kind.as_str())
                    == Some("measured_reference")
                {
                    bound += 1;
                }
            }
            "not_observed" => absent += 1,
            _ => untyped += 1,
        }
    }
    (observed, absent, untyped, bound)
}

// ─────────────────────────── route replay ───────────────────────────

#[test]
fn committed_route_receipt_replays_from_the_committed_carriers() {
    let receipt = route();
    let boundary = boundary();
    let freeze = freeze();

    // The runner's default artifact path is the committed artifact.
    assert_eq!(
        DEFAULT_ROUTE_ARTIFACT_REL,
        "artifacts/parse_compat/native_route_e2e.json"
    );
    assert_eq!(receipt.schema_version, NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION);
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(receipt.locks, required_locks_map());
    assert_eq!(receipt.redaction, Redaction::clean());

    // The declaration binding, re-derived from the declaration bytes.
    assert_eq!(receipt.declaration.manifest, ROUTE_DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, ROUTE_DECLARATION_POINTER);
    assert_eq!(
        receipt.declaration.revision_sha256,
        sha256_prefixed(LAYOUT.as_bytes()),
        "the declaration pin must be the committed declaration bytes"
    );

    // The corpus pin, re-derived from the committed declaration.
    let corpus = declared_route_corpus();
    assert_eq!(
        corpus.document_count, 3,
        "the declared corpus is three documents"
    );
    assert_eq!(
        corpus.page_denominator, 202,
        "the declared denominator is 202 pages"
    );
    assert_eq!(receipt.documents.len(), corpus.documents.len());

    // The freeze, bound by revision and by the metric dictionary.
    let declared_revision = governor_limit_policy()["freeze_revision"]["value"]
        .as_str()
        .expect("a declared revision")
        .to_owned();
    assert_eq!(
        recompute_freeze_revision(),
        declared_revision,
        "the composite revision must recompute from the pinned carrier bytes"
    );
    assert_eq!(
        receipt.limits.freeze_revision,
        recompute_freeze_revision(),
        "the receipt binds the recomputed freeze revision"
    );
    assert_eq!(receipt.limits.freeze_state, "frozen");
    assert_eq!(receipt.limits.tuned_after_scoring, "forbidden");
    assert_eq!(receipt.limits.claim_without_freeze, "forbidden");
    assert_eq!(receipt.limits.silent_clamp, "forbidden");
    assert!(receipt.limits.zero_is_not_a_measurement);
    assert!(receipt.limits.pass_requires_frozen_threshold);
    assert_eq!(
        receipt.limits.direction_vocabulary,
        ROUTE_DIRECTION_VOCABULARY.to_vec()
    );
    let metric_ids: Vec<&str> = receipt
        .limits
        .metrics
        .iter()
        .map(|metric| metric.id.as_str())
        .collect();
    assert_eq!(metric_ids, ROUTE_FROZEN_METRIC_VOCABULARY.to_vec());
    assert_eq!(receipt.limits.metrics, freeze.metrics);

    // The boundary is total and disjoint over P0..P9.
    assert_eq!(boundary.cascade_stages.len(), 10, "P0..P9");
    assert_eq!(boundary.admission_stage, "P0");
    assert_eq!(boundary.in_slice_stages, vec!["P1", "P2"]);
    assert_eq!(boundary.out_of_slice_stages.len(), 7);
    let union: BTreeSet<&str> = boundary
        .in_slice_stages
        .iter()
        .chain(boundary.out_of_slice_stages.iter())
        .map(String::as_str)
        .chain(std::iter::once(boundary.admission_stage.as_str()))
        .collect();
    let cascade: BTreeSet<&str> = boundary.cascade_stages.iter().map(String::as_str).collect();
    assert_eq!(
        union, cascade,
        "every cascade stage is classified exactly once"
    );

    // Every document carries a total, typed stage matrix and a total family matrix.
    let mut matrix_documents: BTreeSet<&str> = BTreeSet::new();
    let mut recomputed_unmeasured = 0u64;
    for document in &receipt.documents {
        let declared = corpus
            .documents
            .iter()
            .find(|declared| declared.id == document.id)
            .expect("no row names a document the declaration does not pin");
        assert!(
            matrix_documents.insert(&document.id),
            "no document appears twice"
        );
        assert_eq!(document.sha256, declared.sha256, "the row is pinned");
        assert_eq!(document.pages, declared.pages);
        assert!(Path::new(&document.source_pdf).is_relative());
        assert!(document.source_pdf.starts_with(&corpus_root_marker()));
        assert_eq!(document.stages.len(), boundary.cascade_stages.len());
        assert_eq!(
            document
                .stages
                .iter()
                .map(|cell| cell.stage.as_str())
                .collect::<Vec<_>>(),
            boundary
                .cascade_stages
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "each document carries every stage exactly once, in cascade order"
        );
        for cell in &document.stages {
            assert!(
                ROUTE_STAGE_STANCE_VOCABULARY.contains(&cell.stance.as_str()),
                "stage `{}` carries an unclosed stance `{}`",
                cell.stage,
                cell.stance
            );
            let in_slice = boundary.in_slice_stages.contains(&cell.stage);
            let out_of_slice = boundary.out_of_slice_stages.contains(&cell.stage);
            let is_admission = cell.stage == boundary.admission_stage;
            match cell.stance.as_str() {
                "admission_boundary" => {
                    assert!(is_admission && cell.reason.is_none());
                }
                "native_measured" => {
                    assert!(in_slice, "an out-of-slice stage was counted as measured");
                    assert!(cell.reason.is_none());
                }
                "not_qualified" => {
                    assert!(in_slice);
                    assert!(cell.reason.is_none());
                }
                "out_of_scope" => {
                    assert!(out_of_slice, "`out_of_scope` on a stage inside the slice");
                    let declared_reason = out_of_slice_reason(&cell.stage);
                    assert!(
                        ROUTE_OUT_OF_SLICE_REASON_VOCABULARY.contains(&declared_reason.as_str()),
                        "the declared out-of-slice reason is outside the closed vocabulary"
                    );
                    assert_eq!(
                        cell.reason.as_deref(),
                        Some(declared_reason.as_str()),
                        "the out-of-scope cell must carry the declared reason"
                    );
                }
                other => panic!("unexpected stage stance `{other}`"),
            }
            if cell.stance != "native_measured" {
                recomputed_unmeasured += 1;
            }
        }
        assert_eq!(document.families.len(), ROUTE_FAMILY_ORDER.len());
        assert_eq!(
            document
                .families
                .iter()
                .map(|cell| cell.family.as_str())
                .collect::<Vec<_>>(),
            ROUTE_FAMILY_ORDER.to_vec()
        );
        for cell in &document.families {
            assert!(
                ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&cell.disposition.as_str()),
                "family `{}` carries an unclosed disposition `{}`",
                cell.family,
                cell.disposition
            );
            assert_ne!(
                cell.disposition, "out_of_scope",
                "a required family must never be spelled out_of_scope"
            );
            match cell.disposition.as_str() {
                "measured" => {
                    assert!(
                        cell.observed_count.is_some(),
                        "a measured cell carries a count"
                    );
                    assert!(cell.reason.is_none());
                }
                "unmet" => {
                    assert!(cell.observed_count.is_none());
                    let reason = cell.reason.as_deref().expect("unmet carries a reason");
                    assert!(ROUTE_UNMET_REASON_VOCABULARY.contains(&reason));
                }
                "refused" => {
                    assert!(cell.observed_count.is_none());
                    let reason = cell.reason.as_deref().expect("refused carries a reason");
                    assert!(ROUTE_REFUSAL_REASON_VOCABULARY.contains(&reason));
                }
                "not_observed" => {
                    assert!(cell.reason.is_none() && cell.observed_count.is_none());
                }
                other => panic!("unexpected family disposition `{other}`"),
            }
        }
    }
    assert_eq!(
        matrix_documents.len(),
        corpus.documents.len(),
        "no document is lost"
    );

    // The reproduced S05 P1 residual is carried typed, never silently dropped.
    assert!(ROUTE_BODY_VERDICT_VOCABULARY.contains(&"not_assessable"));
    assert!(ROUTE_BODY_REASON_VOCABULARY.contains(&"p1_localization_uncertain"));
    let residual_body = &receipt.residuals.body;
    assert_eq!(residual_body.plane_preservation, "not_assessable");
    assert_eq!(residual_body.plane_gap_reason, "p1_localization_uncertain");
    assert_eq!(residual_body.plane_absent_units, 0);
    assert!(!residual_body.plane_d498_reopen_required);
    assert_eq!(
        residual_body, &receipt.documents[0].body,
        "the slice residual reproduces the first document's body axis"
    );
    for document in &receipt.documents {
        assert_eq!(
            document.body.plane_preservation, "not_assessable",
            "the S05 residual is carried, not resolved (D499)"
        );
        assert_eq!(document.body.plane_gap_reason, "p1_localization_uncertain");
        assert!(!document.body.plane_d498_reopen_required);
        assert_eq!(document.body.plane_absent_units, 0);
    }
    assert_eq!(
        receipt.residuals.unmeasured_stages, recomputed_unmeasured,
        "the residual must equal the recomputed typed stage counts"
    );
    assert_eq!(receipt.residuals.unclassified_families, 0);

    // The guard: the counted zero is a measurement under an installed,
    // identity-isolated observer on the real Java seam.
    assert_eq!(receipt.guard.observer.as_str(), GUARD_OBSERVER);
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(receipt.guard.observed_java_calls, 0);

    // The measurement populations stay closed and cover the corpus exactly once.
    let names: BTreeSet<&str> = receipt
        .populations
        .iter()
        .map(|population| population.population.token())
        .collect();
    let population_vocabulary: BTreeSet<&str> =
        ROUTE_POPULATION_VOCABULARY.iter().copied().collect();
    assert!(names.is_subset(&population_vocabulary));
    assert!(
        names.contains("native_route"),
        "the route population is declared"
    );
    let mut pooled_documents: BTreeSet<&str> = BTreeSet::new();
    for population in &receipt.populations {
        assert_eq!(
            population.document_count as usize,
            population.documents.len()
        );
        assert_eq!(
            population.page_denominator,
            population
                .documents
                .iter()
                .map(|document| document.pages)
                .sum::<u64>()
        );
        for identity in &population.documents {
            let declared = corpus
                .documents
                .iter()
                .find(|declared| declared.id == identity.id)
                .expect("no population invents a document");
            assert_eq!(identity.sha256, declared.sha256);
            assert_eq!(identity.pages, declared.pages);
            assert!(
                pooled_documents.insert(&identity.id),
                "populations must stay disjoint"
            );
        }
        for metric in &population.metrics {
            assert!(ROUTE_FROZEN_METRIC_VOCABULARY.contains(&metric.id.as_str()));
            assert!(ROUTE_DIRECTION_VOCABULARY.contains(&metric.direction.as_str()));
        }
    }
    assert_eq!(pooled_documents.len(), corpus.documents.len());

    // Byte-for-byte canonical replay, and redaction of the committed bytes.
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        ROUTE_RECEIPT,
        "the committed artifact must be exactly the canonical serialization"
    );
    assert_route_redacted(ROUTE_RECEIPT).expect("the committed artifact is redacted");
    for marker in LEAK_MARKERS {
        assert!(
            !ROUTE_RECEIPT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }

    // And the whole thing re-reads against the committed sources.
    validate_route_receipt(&receipt, &route_expectation())
        .expect("the committed route receipt must re-validate against the committed carriers");
}

// ─────────────────────────── resource replay ───────────────────────────

#[test]
fn committed_resource_receipt_replays_from_the_committed_carriers() {
    let receipt = same_n();
    let route = route();

    assert_eq!(
        DEFAULT_SAME_N_ARTIFACT_REL,
        "artifacts/parse_compat/native_resource_same_n.json"
    );
    assert_eq!(
        receipt.schema_version,
        NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION
    );
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(receipt.locks, required_locks_map());
    assert_eq!(receipt.redaction, Redaction::clean());

    // Both receipts bind the same declaration and the same T02 freeze.
    assert_eq!(receipt.declaration.manifest, ROUTE_DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, ROUTE_DECLARATION_POINTER);
    assert_eq!(
        receipt.declaration.revision_sha256,
        sha256_prefixed(LAYOUT.as_bytes())
    );
    assert_eq!(
        receipt.limits.freeze_revision,
        recompute_freeze_revision(),
        "the resource receipt binds the recomputed freeze revision"
    );
    assert_eq!(
        receipt.limits, route.limits,
        "both receipts bind the same freeze revision and metric dictionary"
    );

    // The pairing frame is the comparison contract's.
    assert_eq!(receipt.pairing.rule, SAME_N_PAIRING_RULE);
    assert_eq!(receipt.pairing.pooled, SAME_N_POOLED_FORBIDDEN);
    assert_eq!(receipt.pairing.saving_claim, SAME_N_SAVING_CLAIM_FORBIDDEN);
    assert_eq!(receipt.pairing.absence_policy, SAME_N_ABSENCE_POLICY);
    assert_eq!(
        receipt.pairing.mismatch_disposition,
        SAME_N_MISMATCH_HARD_FAILURE
    );

    // The corpus is the committed declaration's, and the same as the route's.
    let corpus = declared_same_n_corpus();
    assert_eq!(
        receipt.corpus, corpus,
        "the corpus must recompute from the declaration"
    );
    assert_eq!(receipt.corpus.document_count, 3);
    assert_eq!(receipt.corpus.page_denominator, 202);
    let route_documents: Vec<(String, String, u64)> = route
        .documents
        .iter()
        .map(|document| (document.id.clone(), document.sha256.clone(), document.pages))
        .collect();
    let resource_documents: Vec<(String, String, u64)> = receipt
        .corpus
        .documents
        .iter()
        .map(|document| (document.id.clone(), document.sha256.clone(), document.pages))
        .collect();
    assert_eq!(
        resource_documents, route_documents,
        "the same-n corpus is exactly the route corpus"
    );

    // Populations are closed, fully bound to the corpus, and never pooled.
    let names: BTreeSet<&str> = receipt
        .populations
        .iter()
        .map(|population| population.population.as_str())
        .collect();
    let population_vocabulary: BTreeSet<&str> =
        SAME_N_POPULATION_VOCABULARY.iter().copied().collect();
    assert_eq!(
        names, population_vocabulary,
        "both populations are declared and no pooled token exists"
    );
    for population in &receipt.populations {
        assert_eq!(population.document_count, receipt.corpus.document_count);
        assert_eq!(population.page_denominator, receipt.corpus.page_denominator);
        assert_eq!(population.documents, receipt.corpus.documents);
        let vocabulary: &[&str] = if population.population == "native" {
            &SAME_N_NATIVE_METRIC_ORDER
        } else {
            &SAME_N_REFERENCE_METRIC_ORDER
        };
        for row in &population.metrics {
            assert!(
                vocabulary.contains(&row.id.as_str()),
                "metric `{}` is outside the `{}` vocabulary",
                row.id,
                population.population
            );
        }
    }

    // Every unmeasured metric is a closed reason, and every measurement is typed.
    let rows = same_n_rows(&receipt);
    let expected_rows = SAME_N_NATIVE_METRIC_ORDER.len()
        + SAME_N_REFERENCE_METRIC_ORDER.len()
        + receipt.documents.len() * SAME_N_REFERENCE_METRIC_ORDER.len();
    assert_eq!(rows.len(), expected_rows);
    let mut observed_reference = 0usize;
    for row in &rows {
        assert!(
            SAME_N_OBSERVATION_STATE_VOCABULARY.contains(&row.state.as_str()),
            "metric `{}` has an untyped state `{}`",
            row.id,
            row.state
        );
        match row.state.as_str() {
            "observed" => {
                assert!(
                    row.value.is_some(),
                    "metric `{}` observed without a value",
                    row.id
                );
                let source = row.source.as_ref().expect("an observed row has a source");
                assert!(row.reason.is_none());
                if source.kind == "measured_reference" {
                    observed_reference += 1;
                }
            }
            _ => {
                assert!(
                    row.value.is_none(),
                    "metric `{}` reports a number as a typed absence",
                    row.id
                );
                let reason = row
                    .reason
                    .as_deref()
                    .expect("a typed absence names a reason");
                assert!(
                    SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason),
                    "metric `{}` names an unknown absence reason `{reason}`",
                    row.id
                );
            }
        }
    }
    assert!(
        observed_reference > 0,
        "at least one reference row is pinned and measured"
    );

    // Observed reference rows name the committed measured work carrier.
    for row in &rows {
        let Some(source) = row.source.as_ref() else {
            continue;
        };
        if source.kind != "measured_reference" {
            continue;
        }
        let manifest = source.manifest.as_deref().expect("a reference manifest");
        assert!(is_repo_relative_path(manifest));
        assert_eq!(manifest, SAME_N_REFERENCE_WORK_REL);
        assert_eq!(
            source.sha256.as_deref().expect("a reference digest"),
            sha256_hex(WORK.as_bytes()),
            "reference row `{}` digest drifted from the committed carrier",
            row.id
        );
        assert_eq!(source.measurement_state.as_deref(), Some("measured"));
        assert!(
            source
                .pointer
                .as_deref()
                .is_some_and(|pointer| !pointer.is_empty()),
            "a pinned reference row names a pointer"
        );
    }

    // The frozen cap metrics carry their ceiling direction; observation-only
    // metrics carry none; and a per-document latency sample is never faked.
    let native = receipt
        .populations
        .iter()
        .find(|population| population.population == "native")
        .expect("a native population");
    for row in &native.metrics {
        if SAME_N_FROZEN_CAP_METRICS.contains(&row.id.as_str()) {
            assert_eq!(row.direction.as_deref(), Some("ceiling"));
        } else {
            assert!(row.direction.is_none());
        }
    }
    let retained = native
        .metrics
        .iter()
        .find(|row| row.id == "peak_retained_bytes")
        .expect("the retained-bytes row");
    assert_eq!(retained.state, "not_observed");
    assert_eq!(retained.value, None);
    assert_eq!(
        retained.reason.as_deref(),
        Some("native_metric_not_measured")
    );
    for document in &receipt.documents {
        assert!(
            document.native_latency_ms.is_some(),
            "document `{}` has no measured native latency",
            document.id
        );
    }
    for row in rows.iter().filter(|row| row.id == "java_cpu_wall_clock_ns") {
        assert_eq!(row.state, "not_observed");
        assert_eq!(row.reason.as_deref(), Some("reference_row_not_same_n"));
    }

    // The closure counters recompute from the typed rows.
    let (observed, absent, untyped, bound) = recompute_same_n_residuals(&receipt);
    assert_eq!(receipt.residuals.observed_metrics, observed);
    assert_eq!(receipt.residuals.absent_metrics, absent);
    assert_eq!(receipt.residuals.untyped_metrics, untyped);
    assert_eq!(receipt.residuals.reference_rows_bound, bound);
    assert_eq!(receipt.residuals.untyped_metrics, 0, "every row is typed");

    // Byte-for-byte canonical replay, redaction, and full re-validation.
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        RESOURCE_RECEIPT,
        "the committed artifact must be exactly the canonical serialization"
    );
    for marker in LEAK_MARKERS {
        assert!(
            !RESOURCE_RECEIPT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }
    validate_same_n_receipt(&receipt, &same_n_expectation())
        .expect("the committed resource receipt must re-validate against the committed carriers");
}

// ─────────────────────────── governor rule ───────────────────────────

#[test]
fn the_governor_rule_and_both_receipts_agree_on_the_committed_data() {
    let gov = gov();
    let route = route();
    let resource = same_n();
    let corpus = declared_route_corpus();

    // The rule names both receipts, and both are required.
    assert_eq!(gov.receipt.path, DEFAULT_ROUTE_ARTIFACT_REL);
    assert_eq!(
        gov.receipt.schema_version,
        NATIVE_ROUTE_RECEIPT_SCHEMA_VERSION
    );
    assert!(gov.receipt.required);
    assert_eq!(gov.resource_receipt.path, DEFAULT_SAME_N_ARTIFACT_REL);
    assert_eq!(
        gov.resource_receipt.schema_version,
        NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION
    );
    assert!(gov.resource_receipt.required);

    // The carriers the rule names are the carriers the receipts bind.
    assert_eq!(gov.declaration.manifest, ROUTE_DECLARATION_MANIFEST_REL);
    assert_eq!(gov.declaration.pointer, ROUTE_DECLARATION_POINTER);
    assert_eq!(route.declaration.manifest, ROUTE_DECLARATION_MANIFEST_REL);
    assert_eq!(route.declaration.pointer, ROUTE_DECLARATION_POINTER);
    assert_eq!(
        resource.declaration.manifest,
        ROUTE_DECLARATION_MANIFEST_REL
    );
    assert_eq!(gov.corpus.manifest, ROUTE_DECLARATION_MANIFEST_REL);
    assert_eq!(gov.corpus.pointer, ROUTE_CORPUS_POINTER);
    assert_eq!(gov.corpus.document_count, corpus.document_count);
    assert_eq!(gov.corpus.page_denominator, corpus.page_denominator);
    assert_eq!(gov.corpus.document_count, 3);
    assert_eq!(gov.corpus.page_denominator, 202);

    // Expected claim values agree with both artifacts.
    assert_eq!(gov.expected.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        gov.expected.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(gov.expected.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(gov.expected.guard_observer, route.guard.observer.as_str());
    assert_eq!(gov.expected.guard_observer, GUARD_OBSERVER);
    assert_eq!(route.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(resource.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);

    // The five closed locks the rule expects are the ones both receipts carry.
    let expected_locks: BTreeMap<String, String> = gov
        .expected
        .locks
        .iter()
        .map(|(lock, value)| (lock.clone(), lock_token(value)))
        .collect();
    assert_eq!(expected_locks, required_locks_map());
    assert_eq!(route.locks, expected_locks);
    assert_eq!(resource.locks, expected_locks);

    // The rule's closed vocabularies are exactly the code's.
    assert_eq!(
        gov.expected.population_vocabulary,
        ROUTE_POPULATION_VOCABULARY.to_vec()
    );
    assert_eq!(gov.expected.native_population, "native_route");
    assert_eq!(
        gov.expected.resource_population_vocabulary,
        SAME_N_POPULATION_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.observation_state_vocabulary,
        SAME_N_OBSERVATION_STATE_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.absence_reason_vocabulary,
        SAME_N_ABSENCE_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.native_metric_vocabulary,
        SAME_N_NATIVE_METRIC_ORDER.to_vec()
    );
    assert_eq!(
        gov.expected.reference_metric_vocabulary,
        SAME_N_REFERENCE_METRIC_ORDER.to_vec()
    );
    assert_eq!(gov.expected.reference_source_kind, "measured_reference");
    assert_eq!(gov.expected.pairing_vocabulary.rule, SAME_N_PAIRING_RULE);
    assert_eq!(
        gov.expected.pairing_vocabulary.pooled,
        SAME_N_POOLED_FORBIDDEN
    );
    assert_eq!(
        gov.expected.pairing_vocabulary.saving_claim,
        SAME_N_SAVING_CLAIM_FORBIDDEN
    );
    assert_eq!(
        gov.expected.pairing_vocabulary.mismatch_disposition,
        SAME_N_MISMATCH_HARD_FAILURE
    );
    assert_eq!(
        gov.expected.body_verdict_vocabulary,
        ROUTE_BODY_VERDICT_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.body_reason_vocabulary,
        ROUTE_BODY_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.stage_stance_vocabulary,
        ROUTE_STAGE_STANCE_VOCABULARY.to_vec()
    );
    assert_eq!(gov.expected.out_of_slice_stance, "out_of_scope");
    assert_eq!(gov.expected.admission_stance, "admission_boundary");
    assert_eq!(
        gov.expected.family_disposition_vocabulary,
        ROUTE_FAMILY_DISPOSITION_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.family_unmet_reason_vocabulary,
        ROUTE_UNMET_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.family_refusal_reason_vocabulary,
        ROUTE_REFUSAL_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(gov.expected.forbidden_family_token, "out_of_scope");
    assert!(
        !ROUTE_FAMILY_DISPOSITION_VOCABULARY.contains(&"out_of_scope"),
        "the forbidden token is not a family disposition"
    );
    assert_eq!(
        gov.expected.residual_fields,
        vec![
            "unmeasured_stages".to_owned(),
            "unclassified_families".to_owned()
        ]
    );

    // The rule's freeze is the module's frozen dictionary.
    assert_eq!(
        governor_limit_policy()["metrics"]
            .as_sequence()
            .expect("metrics")
            .iter()
            .map(|metric| metric.as_str().expect("a metric id").to_owned())
            .collect::<Vec<_>>(),
        ROUTE_FROZEN_METRIC_VOCABULARY.to_vec()
    );
    assert_eq!(
        governor_limit_policy()["direction_vocabulary"]
            .as_sequence()
            .expect("directions")
            .iter()
            .map(|value| value.as_str().expect("a direction").to_owned())
            .collect::<Vec<_>>(),
        ROUTE_DIRECTION_VOCABULARY.to_vec()
    );
    assert_eq!(
        governor_limit_policy()["freeze_revision"]["value"]
            .as_str()
            .expect("the declared revision"),
        recompute_freeze_revision()
    );

    // The closed label vocabulary is thirteen labels, and the committed negative
    // fixture raises every one of them: silence is the failure mode.
    assert_eq!(
        gov.violations.len(),
        13,
        "the route label set is closed at thirteen"
    );
    let fixture: Vec<NegativeEntry> =
        serde_json::from_str(NEGATIVE_FIXTURE).expect("the committed negative fixture parses");
    let labels: Vec<String> = fixture.into_iter().map(|entry| entry.violation).collect();
    let label_set: BTreeSet<&str> = labels.iter().map(String::as_str).collect();
    assert_eq!(
        label_set.len(),
        labels.len(),
        "the negative fixture raises each label once"
    );
    assert_eq!(
        label_set,
        gov.violations
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<&str>>(),
        "the governor vocabulary and the negative fixture label set must be identical"
    );
    assert!(
        gov.violations
            .iter()
            .all(|label| label.starts_with("native_route_")),
        "every route label is namespaced"
    );
    assert_eq!(
        gov.fixtures.negative.expect.require_violations, gov.violations,
        "the fixture expectation must require exactly the registered vocabulary"
    );
    assert_eq!(gov.fixtures.negative.path, NEGATIVE_FIXTURE_REL);
    assert_eq!(gov.fixtures.negative.id, "native_route_broken");
}

// ─────────────────────────── guard provenance ───────────────────────────

#[test]
fn a_counted_zero_requires_an_installed_and_isolated_guard() {
    let expected = route_expectation();
    let committed = route();
    validate_route_receipt(&committed, &expected).expect("the committed receipt is guarded");

    // Negative: the very same zero without an installed guard is refused.
    let mut unguarded = committed.clone();
    unguarded.guard.installed = false;
    assert_eq!(
        validate_route_receipt(&unguarded, &expected).unwrap_err(),
        RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: 0
        }
    );

    // Negative: a guard that rotates the cache identity cannot attribute its zero.
    let mut rotated = committed.clone();
    rotated.guard.identity_isolated = false;
    assert_eq!(
        validate_route_receipt(&rotated, &expected).unwrap_err(),
        RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: 0
        }
    );

    // Negative: a measured Java consult is never a zero.
    let mut consulted = committed.clone();
    consulted.guard.observed_java_calls = 2;
    assert_eq!(
        validate_route_receipt(&consulted, &expected).unwrap_err(),
        RouteReceiptError::ObservedJavaOnRoute { attempts: 2 }
    );

    // Negative: the observer vocabulary is closed at exactly one arm, so a token
    // outside it cannot even be parsed into the receipt. This is the boundary
    // half of `GuardObserverDrift`: the seam cannot drift silently.
    assert_eq!(committed.guard.observer.as_str(), GUARD_OBSERVER);
    let drifted = ROUTE_RECEIPT.replace(
        "\"observer\": \"java_transport_fetch_tei\"",
        "\"observer\": \"unregistered_observer\"",
    );
    assert_ne!(
        drifted, ROUTE_RECEIPT,
        "the committed artifact carries the declared observer token"
    );
    assert!(
        RouteReceipt::parse_json_bytes(drifted.as_bytes()).is_err(),
        "an observer outside the closed vocabulary must be refused at the boundary"
    );
}

#[tokio::test]
async fn the_observer_counts_a_real_java_consult() {
    // The same observer the receipts install, driven through the real seam.
    let counter = JavaInvocationCounter::new();
    let plain = GrobidHttpBackend::new(Some("http://")).with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());

    // An unobserved seam is a typed absence, never a fabricated zero.
    assert_eq!(
        plain.java_invocation_observation(),
        JavaInvocationObservation::NotObserved
    );
    assert_eq!(
        counter.observation(),
        JavaInvocationObservation::Observed { attempts: 0 }
    );

    // Consulting Java on a guaranteed-unreachable endpoint counts the attempt
    // even though no wire exchange completes. `http://` carries an empty
    // authority, so this fails at URL parsing: no socket, no DNS, no race.
    let error = watched
        .fetch_tei(ParseBackendRequest::new(
            "m514-s06-t08",
            b"%PDF-1.4 route-receipt-fixture".to_vec(),
        ))
        .await
        .expect_err("an unroutable endpoint must fail");
    assert_eq!(error.code(), FailureCode::Transport);

    let measured = counter.observation();
    assert_eq!(
        measured,
        JavaInvocationObservation::Observed { attempts: 1 },
        "the attempt is counted before any wire bytes exist"
    );
    let attempts = measured.attempts().expect("an installed guard is observed");
    assert!(attempts > 0, "the counter must be able to grow");

    // Both receipts refuse that measurement: Java was consulted on the corpus.
    let mut route = route();
    route.guard.observed_java_calls = attempts;
    assert_eq!(
        validate_route_receipt(&route, &route_expectation()).unwrap_err(),
        RouteReceiptError::ObservedJavaOnRoute { attempts }
    );
    let mut resource = same_n();
    resource.guard.observed_java_calls = attempts;
    assert_eq!(
        validate_same_n_receipt(&resource, &same_n_expectation()).unwrap_err(),
        SameNReceiptError::ObservedJavaOnRoute { found: attempts }
    );

    // Installing the observer is transport tuning: it never rotates the identity.
    assert_eq!(plain.identity(), watched.identity());
    assert_eq!(plain.profile(), watched.profile());
    assert_ne!(
        plain.identity(),
        plain.clone().with_backend_version("0.9.0-crf").identity(),
        "a real identity input must change the identity, or the isolation is vacuous"
    );
}

// ─────────────────────────── named negative mutations ───────────────────────────

#[test]
fn every_receipt_mutation_fails_by_name() {
    let route_expected = route_expectation();
    let resource_expected = same_n_expectation();

    // ── route: a stale receipt pin ──
    let mut stale_pin = route();
    stale_pin.declaration.revision_sha256 = sha256_prefixed(b"stale declaration");
    assert_eq!(
        validate_route_receipt(&stale_pin, &route_expected).unwrap_err(),
        RouteReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: sha256_prefixed(LAYOUT.as_bytes()),
            found: sha256_prefixed(b"stale declaration"),
        }
    );

    // ── route: a drifted freeze revision ──
    let mut drifted_freeze = route();
    drifted_freeze.limits.freeze_revision = sha256_prefixed(b"other-freeze");
    assert_eq!(
        validate_route_receipt(&drifted_freeze, &route_expected).unwrap_err(),
        RouteReceiptError::StaleFreezeRevision {
            expected: recompute_freeze_revision(),
            found: sha256_prefixed(b"other-freeze"),
        }
    );

    // ── route: an opened lock ──
    let mut opened = route();
    opened
        .locks
        .insert("import_eligible".to_owned(), "true".to_owned());
    assert_eq!(
        validate_route_receipt(&opened, &route_expected).unwrap_err(),
        RouteReceiptError::LockOpened {
            lock: "import_eligible".to_owned(),
            expected: "false".to_owned(),
            found: "true".to_owned(),
        }
    );

    // ── route: a threshold claim ──
    let mut threshold = route();
    threshold.thresholds = "accepted".to_owned();
    assert_eq!(
        validate_route_receipt(&threshold, &route_expected).unwrap_err(),
        RouteReceiptError::ThresholdClaimed {
            found: "accepted".to_owned()
        }
    );

    // ── route: a guard-less observed zero ──
    let mut unguarded = route();
    unguarded.guard.installed = false;
    assert_eq!(
        validate_route_receipt(&unguarded, &route_expected).unwrap_err(),
        RouteReceiptError::GuardMissingForObservedZero {
            observed_java_calls: 0
        }
    );

    // ── route: an out-of-slice stage counted as natively measured ──
    let mut outside = route();
    let document_id = outside.documents[0].id.clone();
    let cell = outside.documents[0]
        .stages
        .iter_mut()
        .find(|cell| cell.stage == "P3")
        .expect("P3 is declared");
    assert_eq!(cell.stance, "out_of_scope");
    cell.stance = "native_measured".to_owned();
    cell.reason = None;
    assert_eq!(
        validate_route_receipt(&outside, &route_expected).unwrap_err(),
        RouteReceiptError::StageOutsideSliceCounted {
            document: document_id,
            stage: "P3".to_owned(),
        }
    );

    // ── route: a required family silently moved out of scope ──
    let mut out_of_scope = route();
    let document_id = out_of_scope.documents[0].id.clone();
    assert_eq!(
        out_of_scope.documents[0].families[0].family,
        ROUTE_FAMILY_ORDER[0]
    );
    out_of_scope.documents[0].families[0].disposition = "out_of_scope".to_owned();
    out_of_scope.documents[0].families[0].reason = None;
    out_of_scope.documents[0].families[0].observed_count = None;
    assert_eq!(
        validate_route_receipt(&out_of_scope, &route_expected).unwrap_err(),
        RouteReceiptError::FamilyOutOfScope {
            document: document_id,
            family: ROUTE_FAMILY_ORDER[0].to_owned(),
            token: "out_of_scope".to_owned(),
        }
    );

    // ── route: an untyped body gap ──
    let mut untyped_body = route();
    let document_id = untyped_body.documents[0].id.clone();
    untyped_body.documents[0].body.plane_preservation = "content_loss".to_owned();
    untyped_body.documents[0].body.plane_d498_reopen_required = false;
    assert_eq!(
        validate_route_receipt(&untyped_body, &route_expected).unwrap_err(),
        RouteReceiptError::BodyGapUntyped {
            document: document_id,
            detail: "the P1 D498 flag does not match the P1 verdict".to_owned(),
        }
    );

    // ── route: a population document that drifted from its pin ──
    let mut mismatched_population = route();
    mismatched_population.populations[0].documents[0].sha256 = "f".repeat(64);
    assert!(matches!(
        validate_route_receipt(&mismatched_population, &route_expected).unwrap_err(),
        RouteReceiptError::PopulationStratificationInvalid { .. }
    ));

    // ── route: a pooled (duplicated) population ──
    let mut pooled_route = route();
    let duplicate = pooled_route.populations[0].clone();
    pooled_route.populations.push(duplicate);
    assert!(matches!(
        validate_route_receipt(&pooled_route, &route_expected).unwrap_err(),
        RouteReceiptError::PoolingClaimed { .. }
    ));

    // ── resource: a drifted freeze revision ──
    let mut drifted_freeze = same_n();
    drifted_freeze.limits.freeze_revision = sha256_prefixed(b"other-freeze");
    assert_eq!(
        validate_same_n_receipt(&drifted_freeze, &resource_expected).unwrap_err(),
        SameNReceiptError::StaleFreezeRevision {
            expected: recompute_freeze_revision(),
            found: sha256_prefixed(b"other-freeze"),
        }
    );

    // ── resource: a corpus document that is not the paired reference set ──
    let mut mismatched_corpus = same_n();
    mismatched_corpus.corpus.documents[0].sha256 = "f".repeat(64);
    assert!(matches!(
        validate_same_n_receipt(&mismatched_corpus, &resource_expected).unwrap_err(),
        SameNReceiptError::Mismatch { .. }
    ));

    // ── resource: an opened lock ──
    let mut opened = same_n();
    opened
        .locks
        .insert("deployed".to_owned(), "true".to_owned());
    assert!(matches!(
        validate_same_n_receipt(&opened, &resource_expected).unwrap_err(),
        SameNReceiptError::LockOpened { .. }
    ));

    // ── resource: a threshold claim ──
    let mut threshold = same_n();
    threshold.thresholds = "accepted".to_owned();
    assert!(matches!(
        validate_same_n_receipt(&threshold, &resource_expected).unwrap_err(),
        SameNReceiptError::ThresholdClaimed { .. }
    ));

    // ── resource: a pooled value ──
    let mut pooled = same_n();
    pooled.pairing.pooled = "allowed".to_owned();
    assert_eq!(
        validate_same_n_receipt(&pooled, &resource_expected).unwrap_err(),
        SameNReceiptError::PooledClaimed {
            found: "allowed".to_owned()
        }
    );

    // ── resource: a saving claim ──
    let mut saving = same_n();
    saving.pairing.saving_claim = "claimed".to_owned();
    assert!(matches!(
        validate_same_n_receipt(&saving, &resource_expected).unwrap_err(),
        SameNReceiptError::SavingClaimed { .. }
    ));

    // ── resource: a metric without an observation state ──
    let mut untyped = same_n();
    let native = untyped
        .populations
        .iter_mut()
        .find(|population| population.population == "native")
        .expect("a native population");
    native.metrics[0].state = "maybe".to_owned();
    assert!(matches!(
        validate_same_n_receipt(&untyped, &resource_expected).unwrap_err(),
        SameNReceiptError::ZeroMasquerade { .. }
    ));

    // ── resource: an unpinned reference source ──
    let mut unpinned = same_n();
    unpinned.documents[0].reference[0]
        .source
        .as_mut()
        .expect("a source")
        .manifest = Some("/var/tmp/leak.json".to_owned());
    assert!(matches!(
        validate_same_n_receipt(&unpinned, &resource_expected).unwrap_err(),
        SameNReceiptError::ReferenceSourceUnpinned { .. }
    ));

    // The committed artifacts are untouched: every mutation was an in-memory
    // clone, and the tracked bytes still canonicalize.
    assert_eq!(
        std::str::from_utf8(
            &route()
                .artifact_json_bytes()
                .expect("the committed route receipt still serializes")
        )
        .expect("artifact is UTF-8"),
        ROUTE_RECEIPT
    );
    assert_eq!(
        std::str::from_utf8(
            &same_n()
                .artifact_json_bytes()
                .expect("the committed resource receipt still serializes")
        )
        .expect("artifact is UTF-8"),
        RESOURCE_RECEIPT
    );
}

// ─────────────────────────── hermeticity ───────────────────────────

#[test]
fn the_audit_reads_no_gitignored_corpus() {
    // Every corpus path the receipts carry is a pin, not an input: this audit
    // resolves none of them, and all of its own inputs are git-tracked DATA.
    // Proving that means scanning this source for the gitignored corpus root and
    // for a filesystem read — either would make the audit unrunnable from a
    // clean checkout.
    let marker = corpus_root_marker();
    assert!(
        !THIS_SOURCE.contains(&marker),
        "the audit must not name the gitignored corpus root"
    );
    assert!(
        !THIS_SOURCE.contains(concat!("fs", "::read")),
        "the audit reads committed bytes, not the filesystem"
    );

    // The corpus pins are repository-relative and stay inside that root; they
    // record what the producers read, and the audit never opens them.
    let route = route();
    for document in &route.documents {
        assert!(Path::new(&document.source_pdf).is_relative());
        assert!(document.source_pdf.starts_with(&marker));
    }
    let resource = same_n();
    for document in &resource.corpus.documents {
        assert!(Path::new(&document.source_pdf).is_relative());
        assert!(document.source_pdf.starts_with(&marker));
    }

    // Both artifact paths are repository-relative.
    assert!(is_repo_relative_path(DEFAULT_ROUTE_ARTIFACT_REL));
    assert!(is_repo_relative_path(DEFAULT_SAME_N_ARTIFACT_REL));

    // The authority locks are carried, not relaxed.
    for receipt_locks in [route.locks, resource.locks] {
        assert_eq!(
            receipt_locks.get("deployed").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            receipt_locks.get("import_eligible").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            receipt_locks.get("auto_promote").map(String::as_str),
            Some("forbidden")
        );
        assert_eq!(
            receipt_locks.get("graph_write").map(String::as_str),
            Some("forbidden")
        );
        assert_eq!(
            receipt_locks.get("production_route").map(String::as_str),
            Some("forbidden")
        );
    }
}
