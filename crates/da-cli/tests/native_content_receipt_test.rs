//! S05/T08 (M514-1lss2s) — hermetic audit of the corpus-bound content receipt.
//!
//! S05/T06 emits `artifacts/parse_compat/native_content_receipt.json`: the
//! native body / section / reading-order / visual / callout bands measured over
//! the declared 3-document, 202-page slice against an independent oracle, with a
//! total group partition and a typed per-family matrix. A receipt is a
//! self-declaration until something else re-reads it, so this file is that
//! independent read: it rebuilds the expectation side from the committed DATA
//! and compares it to the committed artifact, then re-reads the artifact through
//! the validator and proves each named mutation is refused.
//!
//! It is gated on `pdf-layout` because the receipt it audits is produced by a
//! `pdf-layout` runner: producer and audit ride one feature lane, and a default
//! `cargo test --tests` never builds the producer. The architecture gate
//! executes it in the `da-cli feature-gated example contracts` step
//! (`cargo test -p da-cli --features pdf-layout --tests`), and the emitter is
//! provisioned by name in the `da-cli content-receipt example runner` step.
//!
//! The audit reads only git-tracked DATA (the S03 declaration, the native visual
//! heads contract, the reviewed segmentation contract and its source artifact,
//! the preserved-TEI provenance and the three TEI fixtures, the governor sibling
//! check block), the git-tracked artifact, and the real `da-parse` transport
//! seam. It never reads the gitignored `data/sources/**/*.pdf` corpus — a clean
//! checkout does not have it — and never writes the committed artifact: every
//! mutation below is an in-memory clone.
//!
//! Disciplines proven, each with its own negative:
//!
//! 1. **Replay from the carriers, not from the receipt.** The corpus pin
//!    (3 documents / 202 pages), the declaration pin, the oracle pin, the
//!    preservation pins and the reviewed-segmentation pin are recomputed from
//!    the committed bytes; the typed receipt must canonicalize back to the
//!    committed bytes byte for byte.
//! 2. **The governor rule and the receipt agree.** The
//!    `native_content_receipt` block is re-read from the check DATA and must
//!    name the same carriers, the same populations and the same closed
//!    vocabularies the artifact carries; its 11-label vocabulary must be exactly
//!    the set the negative fixture raises.
//! 3. **A counted zero needs a guard.** The zero observed-Java count is only
//!    evidence under an installed, identity-isolated observer; the same receipt
//!    without it is refused. The counter is non-vacuous: a real
//!    `GrobidHttpBackend::fetch_tei` on an unreachable `http://` endpoint yields
//!    `Observed(> 0)`, which the receipt refuses (`ObservedJavaOnSlice`).
//! 4. **The oracle is independent.** Its generator is none of the six native
//!    band generators, its backend is the other generator that produced the
//!    preserved bytes, all four content-band independence requirements accept
//!    it, and a same-generator provenance is refused by each of them.
//! 5. **The partition stays total and stratified.** Every declared document
//!    carries exactly one group assignment (or a typed `bytes_absent`), the
//!    populations stratify exactly that assignment, and the denominator stays
//!    3 documents / 202 pages.
//! 6. **The oracle is not vacuous.** Every required family carries a positive
//!    row somewhere on the slice, and a receipt whose oracle carries only
//!    absences is refused (`OracleVacuous`).
//! 7. **Mutations fail by name.** A substituted population, a truncated group
//!    partition, a stale pin, a threshold claim, a product-coverage claim, an
//!    `out_of_scope` required family, a vacuous oracle and a non-independent
//!    oracle each produce their own typed refusal.
//!
//! Determinism (ADR-051): no network, no clock, no retry. The single transport
//! call uses a malformed `http://` base URL — it fails at URL parsing, so there
//! is no socket, no DNS and no bind/drop race.
//!
//! ## The one leaf a hermetic replay cannot recompute
//!
//! The group partition's `pdf_type` is a *classifier verdict over the pinned PDF
//! bytes*, and those bytes are gitignored: nothing in the committed tree can
//! re-derive `text` from `sha256:ab3a5139…`. The audit therefore re-pins every
//! checkable leaf of the partition (document identity, `pdf_sha256` against the
//! declaration pin, the classifier identity, totality, stratification) and
//! treats the verdict itself as the receipt's declared measurement, exactly as
//! the governor adapter does. This is the same bounded self-declaration T06
//! recorded as a known issue, named here rather than silently trusted.

#![cfg(feature = "pdf-layout")]

use da_cli::native_content_receipt::{
    CONTENT_BODY_REASON_VOCABULARY, CONTENT_BODY_VERDICT_VOCABULARY, CONTENT_CORPUS_POINTER,
    CONTENT_DECLARATION_POINTER, CONTENT_FAMILY_ORDER, CONTENT_ORACLE_MANIFEST_REL,
    CONTENT_POPULATION_ORDER, ContentExpectation, ContentFamily, ContentPopulationId,
    ContentReceipt, ContentReceiptError, DEFAULT_CONTENT_ARTIFACT_REL,
    NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION, SEGMENTATION_ORACLE_MANIFEST_REL,
    SEGMENTATION_ORACLE_SOURCE_REL, assert_content_redacted, content_sha256_hex,
    content_sha256_prefixed, validate_content_receipt,
};
use da_cli::native_document_group::{
    DOCUMENT_BYTES_ABSENT_VOCABULARY, DOCUMENT_GROUP_PARTITION_STATE_VOCABULARY,
    DOCUMENT_GROUP_SOURCE_VOCABULARY, DOCUMENT_GROUP_VOCABULARY, DocumentGroupEvidence,
    DocumentGroupPartition, DocumentGroupSource, FORBIDDEN_REQUIRED_FAMILY_TOKEN,
    REQUIRED_FAMILY_DISPOSITION_VOCABULARY, REQUIRED_FAMILY_UNMET_VOCABULARY,
};
use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, DECLARATION_MANIFEST_REL, LEAK_MARKERS,
    PRODUCT_COVERAGE_EFFECT_NONE, REQUIRED_LOCKS, Redaction, THRESHOLDS_NOT_ACCEPTED,
    normalize_document_identity, sha256_hex, sha256_prefixed,
};
use da_parse::{
    BodyOracleIndependence, BodyOracleProvenance, FailureCode, GrobidHttpBackend, GrobidProfile,
    JavaInvocationCounter, JavaInvocationObservation, NATIVE_BODY_TEXT_GENERATOR_ID,
    NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID, NATIVE_HEADER_INPUT_GENERATOR_ID,
    NATIVE_SECTION_SUT_GENERATOR_ID, NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID,
    NativeCalloutOracleProvenance, NativeVisualOracleProvenance, ParseBackend, ParseBackendRequest,
    require_independent_body_oracle, require_independent_callout_oracle,
    require_independent_section_oracle, require_independent_visual_oracle,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ─────────────────────────── committed carriers ───────────────────────────

/// The content receipt under audit, as committed.
const RECEIPT_ARTIFACT: &str =
    include_str!("../../../artifacts/parse_compat/native_content_receipt.json");
/// The S03 slice declaration and corpus pin.
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
/// The native visual-heads contract (closed head vocabulary + page topology).
const VISUAL_HEADS: &str = include_str!("../../../data/parse/native_visual_heads.yaml");
/// The reviewed segmentation contract and its source artifact.
const SEGMENTATION: &str = include_str!("../../../data/parse/document_segmentation.yaml");
const SEGMENTATION_SOURCE: &str =
    include_str!("../../../artifacts/parse_compat/document_segmentation_oracle_source.json");
/// The preserved-TEI oracle pins and the three fixtures.
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");
const HUB_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml");
const RLM_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml");
const RECA_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml");
/// The governor check block that governs this receipt.
const GOV_CHECK: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");

/// Identity of the classifier that produced the group partition. It mirrors the
/// emitter's typed constants; a drift on either side is a test failure.
const CONTENT_CLASSIFIER_ID: &str = "pdf-inspector";
const CONTENT_CLASSIFIER_VERSION: &str = "0.1.0";
/// The backend that produced the preserved oracle bytes.
const ORACLE_BACKEND_ID: &str = "java-grobid-crf";
/// The guard observer the contract requires.
const GUARD_OBSERVER: &str = "java_transport_fetch_tei";

// ─────────────────────────── DATA mirrors ───────────────────────────

#[derive(Debug, Deserialize)]
struct LayoutDocument {
    id: String,
    source_pdf: String,
    sha256: String,
    pages: u64,
}

#[derive(Debug, Deserialize)]
struct LayoutCorpus {
    id: String,
    corpus_n: u64,
    page_denominator: u64,
    documents: Vec<LayoutDocument>,
}

#[derive(Debug, Deserialize)]
struct Layout {
    corpus: LayoutCorpus,
}

#[derive(Debug, Deserialize)]
struct VisualHeadsAuthority {
    exact_document_count: u64,
    page_denominator: u64,
}

#[derive(Debug, Deserialize)]
struct VisualHeadsBlock {
    closed_labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VisualHeadsDocument {
    id: String,
    sha256: String,
    pages: u64,
}

#[derive(Debug, Deserialize)]
struct VisualHeads {
    source_authority: VisualHeadsAuthority,
    heads: VisualHeadsBlock,
    source_documents: Vec<VisualHeadsDocument>,
}

#[derive(Debug, Deserialize)]
struct SegmentationOracle {
    source_artifact: String,
    source_artifact_sha256: String,
}

#[derive(Debug, Deserialize)]
struct Segmentation {
    oracle: SegmentationOracle,
}

#[derive(Debug, Deserialize)]
struct ProvenanceFixture {
    id: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct Provenance {
    backend: String,
    fixtures: Vec<ProvenanceFixture>,
}

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
}

#[derive(Debug, Deserialize)]
struct GovOracle {
    manifest: String,
    pointer: String,
    segmentation_manifest: String,
    segmentation_source: String,
}

#[derive(Debug, Deserialize)]
struct GovPopulations {
    document_count: u64,
    page_denominator: u64,
}

#[derive(Debug, Deserialize)]
struct GovExpected {
    claim_scope: String,
    product_coverage_effect: String,
    thresholds: String,
    guard_observer: String,
    families: Vec<String>,
    required_families: Vec<String>,
    family_dispositions: Vec<String>,
    unmet_reasons: Vec<String>,
    forbidden_family_token: String,
    body_verdicts: Vec<String>,
    body_reasons: Vec<String>,
    group_vocabulary: Vec<String>,
    group_source_vocabulary: Vec<String>,
    partition_states: Vec<String>,
    bytes_absent_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GovNegativeExpect {
    require_violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GovNegative {
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
    corpus: GovCorpus,
    oracle: GovOracle,
    populations: GovPopulations,
    expected: GovExpected,
    violations: Vec<String>,
    fixtures: GovFixtures,
}

#[derive(Debug, Deserialize)]
struct GovCheck {
    native_content_receipt: GovRule,
}

// ─────────────────────────── helpers ───────────────────────────

fn receipt() -> ContentReceipt {
    ContentReceipt::parse_json_bytes(RECEIPT_ARTIFACT.as_bytes())
        .expect("the committed content receipt must parse")
}

fn layout() -> Layout {
    serde_yaml::from_str(LAYOUT).expect("the committed slice declaration must parse")
}

fn visual_heads() -> VisualHeads {
    serde_yaml::from_str(VISUAL_HEADS).expect("the committed visual-heads contract must parse")
}

fn segmentation() -> Segmentation {
    serde_yaml::from_str(SEGMENTATION).expect("the committed segmentation contract must parse")
}

fn provenance() -> Provenance {
    serde_yaml::from_str(PROVENANCE).expect("the committed provenance must parse")
}

fn gov() -> GovRule {
    serde_yaml::from_str::<GovCheck>(GOV_CHECK)
        .expect("the committed check must parse")
        .native_content_receipt
}

fn tei_bytes_for(document_id: &str) -> &'static str {
    // `hub_2507.19457` (provenance) and `hub-2507.19457` (declaration) normalize
    // to the same key, so the arXiv id substring identifies the fixture.
    if document_id.contains("2507.19457") {
        HUB_TEI
    } else if document_id.contains("2512.24601") {
        RLM_TEI
    } else if document_id.contains("2605.26525") {
        RECA_TEI
    } else {
        panic!("no preserved TEI fixture for `{document_id}`")
    }
}

/// Parse a family token through the closed vocabulary, so a token the DATA
/// carries but the type cannot represent is a failure rather than a silent skip.
fn content_family(token: &str) -> ContentFamily {
    serde_json::from_value(serde_json::Value::String(token.to_owned()))
        .unwrap_or_else(|_| panic!("`{token}` is not a member of the content-family vocabulary"))
}

/// The required families, in the order the governor declares them.
fn required_families() -> Vec<ContentFamily> {
    gov()
        .expected
        .required_families
        .iter()
        .map(|token| content_family(token))
        .collect()
}

/// The declared corpus, rebuilt from the committed declaration bytes.
fn declared_corpus() -> LayoutCorpus {
    layout().corpus
}

/// The total group partition, rebuilt from the committed corpus pins and the
/// receipt's declared classifier verdict.
///
/// Every leaf that *can* be recomputed is: the document set and the `pdf_sha256`
/// leaf come from the declaration, the classifier identity from the typed
/// constants. The verdict itself is the classifier's measurement over bytes the
/// audit must not open (see the module docstring).
fn expected_partition(receipt: &ContentReceipt, corpus: &LayoutCorpus) -> DocumentGroupPartition {
    let declared_ids: Vec<String> = corpus
        .documents
        .iter()
        .map(|document| document.id.clone())
        .collect();
    let mut assignments = Vec::with_capacity(declared_ids.len());
    for document in &corpus.documents {
        let evidence = receipt
            .partition
            .assignments
            .iter()
            .find(|evidence| evidence.document_id == document.id)
            .unwrap_or_else(|| {
                panic!(
                    "the receipt carries no group evidence for `{}`",
                    document.id
                )
            });
        match &evidence.source {
            DocumentGroupSource::Measured(measured) => {
                assert_eq!(
                    measured.pdf_sha256, document.sha256,
                    "group evidence for `{}` is not pinned to the declared bytes",
                    document.id
                );
                assert_eq!(measured.classifier_id, CONTENT_CLASSIFIER_ID);
                assert_eq!(measured.classifier_version, CONTENT_CLASSIFIER_VERSION);
                assignments.push(DocumentGroupEvidence::measured(
                    document.id.clone(),
                    measured.pdf_type.clone(),
                    CONTENT_CLASSIFIER_ID,
                    CONTENT_CLASSIFIER_VERSION,
                    document.sha256.clone(),
                ));
            }
            DocumentGroupSource::BytesAbsent(absent) => {
                assignments.push(DocumentGroupEvidence::bytes_absent(
                    document.id.clone(),
                    absent.reason,
                ));
            }
        }
    }
    DocumentGroupPartition::assemble(corpus.id.clone(), &declared_ids, assignments)
        .expect("the committed declaration partitions totally")
}

/// The committed sources the receipt is re-read against.
fn expectation() -> ContentExpectation {
    let receipt = receipt();
    let corpus = declared_corpus();
    let mut oracle_fixture_sha256 = BTreeMap::new();
    for fixture in provenance().fixtures {
        oracle_fixture_sha256.insert(
            normalize_document_identity(&fixture.id),
            sha256_hex(tei_bytes_for(&fixture.id).as_bytes()),
        );
    }
    ContentExpectation {
        partition: expected_partition(&receipt, &corpus),
        declaration_revision_sha256: sha256_prefixed(LAYOUT.as_bytes()),
        oracle_revision_sha256: sha256_prefixed(PROVENANCE.as_bytes()),
        oracle_fixture_sha256,
        segmentation_revision_sha256: sha256_prefixed(SEGMENTATION.as_bytes()),
        segmentation_source_sha256: sha256_prefixed(SEGMENTATION_SOURCE.as_bytes()),
        required_families: required_families(),
    }
}

/// Every family cell of the receipt, in stratum/matrix order.
fn family_cells(
    receipt: &ContentReceipt,
) -> Vec<&da_cli::native_content_receipt::ContentFamilyCell> {
    receipt
        .populations
        .iter()
        .flat_map(|population| population.matrix.iter())
        .flat_map(|document| document.families.iter())
        .collect()
}

// ─────────────────────────── replay ───────────────────────────

#[test]
fn committed_content_receipt_replays_from_the_committed_carriers() {
    let receipt = receipt();
    let corpus = declared_corpus();
    let heads = visual_heads();
    let segmentation = segmentation();
    let provenance = provenance();
    let gov = gov();

    // The runner's default artifact path is the committed artifact.
    assert_eq!(
        DEFAULT_CONTENT_ARTIFACT_REL,
        "artifacts/parse_compat/native_content_receipt.json"
    );
    assert_eq!(
        receipt.schema_version,
        NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION
    );
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(receipt.locks.get(lock).map(String::as_str), Some(value));
    }
    assert_eq!(receipt.locks.len(), REQUIRED_LOCKS.len());
    assert_eq!(receipt.redaction, Redaction::clean());

    // The declaration binding, re-derived from the declaration bytes.
    assert_eq!(corpus.corpus_n, 3, "the declared corpus is three documents");
    assert_eq!(
        corpus.page_denominator, 202,
        "the declared denominator is 202 pages"
    );
    assert_eq!(
        corpus
            .documents
            .iter()
            .map(|document| document.pages)
            .sum::<u64>(),
        corpus.page_denominator
    );
    assert_eq!(receipt.declaration.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, CONTENT_DECLARATION_POINTER);
    assert_eq!(
        receipt.declaration.revision_sha256,
        content_sha256_prefixed(LAYOUT.as_bytes()),
        "the declaration pin must be the committed declaration bytes"
    );

    // The corpus DATA agrees with the other committed carriers.
    assert_eq!(heads.source_authority.exact_document_count, corpus.corpus_n);
    assert_eq!(
        heads.source_authority.page_denominator,
        corpus.page_denominator
    );
    assert_eq!(heads.source_documents.len(), corpus.documents.len());
    for (head, declared) in heads.source_documents.iter().zip(&corpus.documents) {
        assert_eq!(head.id, declared.id);
        assert_eq!(head.sha256, declared.sha256);
        assert_eq!(head.pages, declared.pages);
    }
    for document in &corpus.documents {
        assert!(Path::new(&document.source_pdf).is_relative());
        assert!(document.source_pdf.starts_with("data/sources/"));
        assert_eq!(document.sha256.len(), 64, "the pin is a bare sha256");
    }

    // The group partition: total, typed, and pinned to the declared corpus.
    assert_eq!(receipt.partition.corpus_id, corpus.id);
    assert_eq!(
        receipt.partition.declared_document_count as u64,
        corpus.corpus_n
    );
    assert_eq!(
        receipt.partition.state,
        da_cli::native_document_group::DocumentGroupPartitionState::Partitioned
    );
    let mut assigned: BTreeSet<&str> = BTreeSet::new();
    for evidence in &receipt.partition.assignments {
        evidence
            .validate()
            .expect("every committed assignment is typed and self-consistent");
        let declared = corpus
            .documents
            .iter()
            .find(|document| document.id == evidence.document_id)
            .expect("no assignment names a document the declaration does not pin");
        assert!(
            assigned.insert(&evidence.document_id),
            "no duplicate assignment"
        );
        match &evidence.source {
            DocumentGroupSource::Measured(measured) => {
                assert_eq!(
                    measured.pdf_sha256, declared.sha256,
                    "a measured assignment must be pinned to the declared PDF bytes"
                );
                assert_eq!(measured.classifier_id, CONTENT_CLASSIFIER_ID);
                assert_eq!(measured.classifier_version, CONTENT_CLASSIFIER_VERSION);
                assert!(
                    DOCUMENT_GROUP_VOCABULARY
                        .contains(&evidence.group.expect("measured carries a group").token()),
                    "a measured assignment's group is outside the closed vocabulary"
                );
            }
            DocumentGroupSource::BytesAbsent(absent) => {
                assert!(evidence.group.is_none(), "a typed absence carries no group");
                assert!(DOCUMENT_BYTES_ABSENT_VOCABULARY.contains(&absent.reason.token()));
            }
        }
    }
    assert_eq!(
        assigned.len(),
        corpus.documents.len(),
        "the partition is total"
    );

    // The group-stratified populations: one stratum per work present, no pooling.
    assert!(!receipt.pooling.group_strata_pooled);
    let mut populations_by_id: Vec<ContentPopulationId> = receipt
        .populations
        .iter()
        .map(|population| population.population)
        .collect();
    populations_by_id.sort();
    let mut canonical_present: Vec<ContentPopulationId> = CONTENT_POPULATION_ORDER.to_vec();
    canonical_present.retain(|stratum| populations_by_id.contains(stratum));
    assert_eq!(
        populations_by_id, canonical_present,
        "strata are canonical and unique"
    );
    let mut total_documents = 0u64;
    let mut total_pages = 0u64;
    let mut matrix_documents: BTreeSet<&str> = BTreeSet::new();
    for population in &receipt.populations {
        assert_eq!(
            population.document_count as usize,
            population.documents.len()
        );
        assert_eq!(population.matrix.len(), population.documents.len());
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
                .find(|document| document.id == identity.id)
                .expect("no stratum invents a document");
            assert_eq!(identity.sha256, declared.sha256);
            assert_eq!(identity.pages, declared.pages);
        }
        for document in &population.matrix {
            assert!(
                matrix_documents.insert(&document.id),
                "no document appears twice"
            );
            // The stratum of a matrix row is the stratum of its own evidence.
            let evidence = receipt
                .partition
                .assignments
                .iter()
                .find(|evidence| evidence.document_id == document.id)
                .expect("every matrix row has group evidence");
            assert_eq!(
                ContentPopulationId::of_evidence(evidence),
                population.population,
                "a matrix row was pooled into the wrong stratum"
            );
            assert_eq!(document.group, *evidence, "the row and the partition agree");
        }
        total_documents += population.document_count;
        total_pages += population.page_denominator;
    }
    assert_eq!(
        matrix_documents.len(),
        corpus.documents.len(),
        "no document is lost"
    );
    assert_eq!(
        total_documents, corpus.corpus_n,
        "the denominator stays 3 documents"
    );
    assert_eq!(
        total_pages, corpus.page_denominator,
        "the denominator stays 202 pages"
    );

    // The family matrix: canonical, complete, typed, and non-vacuous.
    let closed_families: BTreeSet<String> = gov.expected.families.iter().cloned().collect();
    assert_eq!(
        family_cells(&receipt)
            .iter()
            .map(|cell| cell.family.token())
            .collect::<BTreeSet<_>>(),
        closed_families
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
    );
    for label in &heads.heads.closed_labels {
        assert!(
            closed_families.contains(label),
            "the visual-heads closed label `{label}` is not a receipt family"
        );
    }
    for population in &receipt.populations {
        for document in &population.matrix {
            assert_eq!(document.families.len(), CONTENT_FAMILY_ORDER.len());
            assert_eq!(
                document
                    .families
                    .iter()
                    .map(|cell| cell.family)
                    .collect::<Vec<_>>(),
                CONTENT_FAMILY_ORDER.to_vec(),
                "each document carries every family exactly once, in canonical order"
            );
            for cell in &document.families {
                assert!(
                    gov.expected.family_dispositions.contains(&cell.disposition),
                    "disposition `{}` is outside the closed vocabulary",
                    cell.disposition
                );
                match cell.disposition.as_str() {
                    "measured" => {
                        assert!(
                            cell.observed_count.is_some(),
                            "a measured cell carries a count"
                        );
                        assert!(cell.unmet_reason.is_none());
                    }
                    "unmet" => {
                        assert!(cell.observed_count.is_none());
                        let reason = cell
                            .unmet_reason
                            .as_deref()
                            .expect("unmet carries a reason");
                        assert!(
                            gov.expected.unmet_reasons.contains(&reason.to_owned()),
                            "unmet reason `{reason}` is outside the closed vocabulary"
                        );
                    }
                    "not_observed" => {
                        assert!(cell.observed_count.is_none());
                        assert!(cell.unmet_reason.is_none());
                    }
                    other => panic!("unexpected disposition `{other}`"),
                }
                // The forbidden token is not a vocabulary member and never appears.
                assert_ne!(cell.disposition, FORBIDDEN_REQUIRED_FAMILY_TOKEN);
                assert_ne!(
                    cell.unmet_reason.as_deref(),
                    Some(FORBIDDEN_REQUIRED_FAMILY_TOKEN),
                    "a required family must never be spelled out_of_scope"
                );
            }
        }
    }

    // The body axis: closed vocabulary, and a P1 loss is exactly the D498 obligation.
    for document in receipt.populations.iter().flat_map(|p| p.matrix.iter()) {
        let body = &document.body;
        for (field, token) in [
            ("plane_preservation", body.plane_preservation.as_str()),
            ("body_region", body.body_region.as_str()),
        ] {
            assert!(
                CONTENT_BODY_VERDICT_VOCABULARY.contains(&token),
                "body axis `{field}` carries `{token}`, outside its vocabulary"
            );
        }
        assert!(CONTENT_BODY_REASON_VOCABULARY.contains(&body.body_reason.as_str()));
        assert_eq!(
            body.plane_preservation == "content_loss",
            body.plane_d498_reopen_required,
            "the P1 verdict and the D498 flag must agree"
        );
        if body.plane_absent_units > 0 {
            assert!(
                body.plane_d498_reopen_required,
                "absent character material requires a recorded D498 obligation"
            );
        }
    }

    // The oracle: identity, independence flags, and pins recomputed from bytes.
    assert_eq!(receipt.oracle.manifest, CONTENT_ORACLE_MANIFEST_REL);
    assert_eq!(
        receipt.oracle.revision_sha256,
        content_sha256_prefixed(PROVENANCE.as_bytes())
    );
    assert_eq!(receipt.oracle.backend, provenance.backend);
    assert_eq!(receipt.oracle.backend, ORACLE_BACKEND_ID);
    assert!(receipt.oracle.independent);
    assert!(receipt.oracle.same_generator_refused);
    assert_eq!(
        receipt.oracle.segmentation_manifest,
        SEGMENTATION_ORACLE_MANIFEST_REL
    );
    assert_eq!(
        receipt.oracle.segmentation_revision_sha256,
        content_sha256_prefixed(SEGMENTATION.as_bytes())
    );
    assert_eq!(
        receipt.oracle.segmentation_source,
        SEGMENTATION_ORACLE_SOURCE_REL
    );
    assert_eq!(
        receipt.oracle.segmentation_source_sha256,
        content_sha256_prefixed(SEGMENTATION_SOURCE.as_bytes())
    );
    // The reviewed segmentation contract pins *its own* source artifact, and the
    // receipt's pin is that recomputed digest — a DATA-to-DATA cross-check that
    // does not read the receipt.
    assert_eq!(
        segmentation.oracle.source_artifact,
        SEGMENTATION_ORACLE_SOURCE_REL
    );
    assert_eq!(
        content_sha256_hex(SEGMENTATION_SOURCE.as_bytes()),
        segmentation.oracle.source_artifact_sha256,
        "the segmentation source drifted from the reviewed contract pin"
    );
    assert_eq!(
        content_sha256_prefixed(SEGMENTATION_SOURCE.as_bytes()),
        receipt.oracle.segmentation_source_sha256
    );
    assert_eq!(receipt.oracle.documents.len(), corpus.documents.len());
    for fixture in &receipt.oracle.documents {
        let pin = provenance
            .fixtures
            .iter()
            .find(|pin| {
                normalize_document_identity(&pin.id)
                    == normalize_document_identity(&fixture.document_id)
            })
            .expect("the receipt names only pinned oracle documents");
        assert_eq!(pin.sha256, sha256_hex(tei_bytes_for(&pin.id).as_bytes()));
        assert_eq!(
            fixture.sha256,
            sha256_hex(tei_bytes_for(&fixture.document_id).as_bytes())
        );
        assert!(Path::new(&fixture.fixture_path).is_relative());
    }

    // The guard: the counted zero is a measurement under an installed,
    // identity-isolated observer on the real Java seam.
    assert_eq!(receipt.guard.observer.as_str(), GUARD_OBSERVER);
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(receipt.guard.observed_java_calls, 0);
    assert_eq!(receipt.redaction, Redaction::clean());

    // Byte-for-byte canonical replay, and redaction of the bytes.
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        RECEIPT_ARTIFACT,
        "the committed artifact must be exactly the canonical serialization"
    );
    assert_content_redacted(RECEIPT_ARTIFACT).expect("the committed artifact is redacted");
    for marker in LEAK_MARKERS {
        assert!(
            !RECEIPT_ARTIFACT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }

    // And the whole thing re-reads against the committed sources.
    validate_content_receipt(&receipt, &expectation())
        .expect("the committed receipt must re-validate against the committed carriers");
}

// ─────────────────────────── governor rule ───────────────────────────

#[test]
fn the_governor_rule_and_the_receipt_agree_on_the_committed_data() {
    let gov = gov();
    let receipt = receipt();
    let corpus = declared_corpus();

    // The rule names this receipt, and the receipt is the rule's artifact.
    assert_eq!(gov.receipt.path, DEFAULT_CONTENT_ARTIFACT_REL);
    assert_eq!(
        gov.receipt.schema_version,
        NATIVE_CONTENT_RECEIPT_SCHEMA_VERSION
    );
    assert!(gov.receipt.required, "the rule requires the receipt");

    // The carriers the rule names are the carriers the receipt binds.
    assert_eq!(gov.declaration.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(gov.declaration.pointer, CONTENT_DECLARATION_POINTER);
    assert_eq!(receipt.declaration.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, CONTENT_DECLARATION_POINTER);
    assert_eq!(gov.corpus.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(gov.corpus.pointer, CONTENT_CORPUS_POINTER);
    assert_eq!(gov.oracle.manifest, CONTENT_ORACLE_MANIFEST_REL);
    assert_eq!(gov.oracle.pointer, "fixtures");
    assert_eq!(
        gov.oracle.segmentation_manifest,
        SEGMENTATION_ORACLE_MANIFEST_REL
    );
    assert_eq!(
        gov.oracle.segmentation_source,
        SEGMENTATION_ORACLE_SOURCE_REL
    );

    // The rule's population equals the declaration's.
    assert_eq!(gov.populations.document_count, corpus.corpus_n);
    assert_eq!(gov.populations.page_denominator, corpus.page_denominator);
    assert_eq!(gov.populations.document_count, 3);
    assert_eq!(gov.populations.page_denominator, 202);

    // Expected claim values agree with the artifact.
    assert_eq!(gov.expected.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        gov.expected.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(gov.expected.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(gov.expected.guard_observer, receipt.guard.observer.as_str());
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);

    // The rule's closed vocabularies are exactly the code's.
    assert_eq!(
        gov.expected.families,
        CONTENT_FAMILY_ORDER
            .iter()
            .map(|family| family.token().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        gov.expected.family_dispositions,
        REQUIRED_FAMILY_DISPOSITION_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.unmet_reasons,
        REQUIRED_FAMILY_UNMET_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.forbidden_family_token,
        FORBIDDEN_REQUIRED_FAMILY_TOKEN
    );
    assert_eq!(
        gov.expected.body_verdicts,
        CONTENT_BODY_VERDICT_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.body_reasons,
        CONTENT_BODY_REASON_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.group_vocabulary,
        DOCUMENT_GROUP_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.group_source_vocabulary,
        DOCUMENT_GROUP_SOURCE_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.partition_states,
        DOCUMENT_GROUP_PARTITION_STATE_VOCABULARY.to_vec()
    );
    assert_eq!(
        gov.expected.bytes_absent_reasons,
        DOCUMENT_BYTES_ABSENT_VOCABULARY.to_vec()
    );
    // The forbidden token is not a member of any closed vocabulary the rule lists.
    assert!(
        !gov.expected
            .family_dispositions
            .iter()
            .any(|token| token == FORBIDDEN_REQUIRED_FAMILY_TOKEN)
    );
    assert!(
        !gov.expected
            .unmet_reasons
            .iter()
            .any(|token| token == FORBIDDEN_REQUIRED_FAMILY_TOKEN)
    );

    // Required families are a subset of the family vocabulary, and each of them
    // carries a positive oracle row on the committed slice.
    for family in &gov.expected.required_families {
        assert!(
            gov.expected.families.contains(family),
            "required family `{family}` is not in the family vocabulary"
        );
        let positive: u64 = family_cells(&receipt)
            .iter()
            .filter(|cell| cell.family.token() == family)
            .map(|cell| cell.oracle_positive_rows)
            .sum();
        assert!(
            positive > 0,
            "required family `{family}` has no positive oracle row"
        );
    }

    // The closed label vocabulary is eleven labels, and the negative fixture
    // raises every one of them: silence is the failure mode.
    assert_eq!(
        gov.violations.len(),
        11,
        "the content label set is closed at eleven"
    );
    let required = gov.fixtures.negative.expect.require_violations;
    assert_eq!(required.len(), 11);
    for label in &gov.violations {
        assert!(
            required.contains(label),
            "the negative fixture omits `{label}`"
        );
    }
    for label in &required {
        assert!(
            gov.violations.contains(label),
            "the fixture requires an unregistered `{label}`"
        );
    }
}

// ─────────────────────────── guard provenance ───────────────────────────

#[test]
fn a_counted_zero_requires_an_installed_and_isolated_guard() {
    let expected = expectation();
    let committed = receipt();
    validate_content_receipt(&committed, &expected).expect("the committed receipt is guarded");

    // Negative: the very same zero without an installed guard is refused.
    let mut unguarded = committed.clone();
    unguarded.guard.installed = false;
    assert_eq!(
        validate_content_receipt(&unguarded, &expected).unwrap_err(),
        ContentReceiptError::GuardNotInstalled
    );

    // Negative: a guard that rotates the cache identity cannot attribute its zero.
    let mut rotated = committed.clone();
    rotated.guard.identity_isolated = false;
    assert_eq!(
        validate_content_receipt(&rotated, &expected).unwrap_err(),
        ContentReceiptError::GuardIdentityNotIsolated
    );

    // Negative: the observer vocabulary is closed at exactly one arm, so a token
    // outside it cannot even be parsed into a receipt. This is the boundary half
    // of the `GuardObserverDrift` refusal: the seam cannot drift silently.
    assert_eq!(committed.guard.observer.as_str(), GUARD_OBSERVER);
    let drifted_json = RECEIPT_ARTIFACT.replace(
        "\"observer\": \"java_transport_fetch_tei\"",
        "\"observer\": \"unregistered_observer\"",
    );
    assert_ne!(
        drifted_json, RECEIPT_ARTIFACT,
        "the committed artifact carries the declared observer token"
    );
    assert!(
        ContentReceipt::parse_json_bytes(drifted_json.as_bytes()).is_err(),
        "an observer outside the closed vocabulary must be refused at the boundary"
    );

    // Negative: a measured Java consult is never a zero.
    let mut consulted = committed.clone();
    consulted.guard.observed_java_calls = 1;
    assert_eq!(
        validate_content_receipt(&consulted, &expected).unwrap_err(),
        ContentReceiptError::ObservedJavaOnSlice { attempts: 1 }
    );
}

#[tokio::test]
async fn the_observer_counts_a_real_java_consult() {
    // The same observer the receipt installs, driven through the real seam.
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
            "m514-s05-t08",
            b"%PDF-1.4 content-receipt-fixture".to_vec(),
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
    assert_eq!(watched.java_invocation_observation(), measured);
    let attempts = measured.attempts().expect("an installed guard is observed");
    assert!(attempts > 0, "the counter must be able to grow");

    // The receipt refuses that measurement: Java was consulted on the slice.
    let mut receipt = receipt();
    receipt.guard.observed_java_calls = attempts;
    assert_eq!(
        validate_content_receipt(&receipt, &expectation()).unwrap_err(),
        ContentReceiptError::ObservedJavaOnSlice { attempts }
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

// ─────────────────────────── oracle independence ───────────────────────────

#[test]
fn the_oracle_is_independent_of_the_sut_bands() {
    let receipt = receipt();
    let provenance = provenance();

    // The oracle generator is none of the native band generators.
    let generator = receipt.oracle.generator_id.clone();
    for sut in [
        NATIVE_BODY_TEXT_GENERATOR_ID,
        NATIVE_SECTION_SUT_GENERATOR_ID,
        NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID,
        NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID,
    ] {
        assert_ne!(generator, sut, "the oracle is the `{sut}` SUT band");
    }
    // ...and not the header/bibliography bands either.
    assert_ne!(generator, NATIVE_HEADER_INPUT_GENERATOR_ID);
    assert_ne!(generator, da_parse::NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID);

    // The oracle is built from the preserved bytes the *other* generator wrote.
    assert_eq!(provenance.backend, ORACLE_BACKEND_ID);
    assert_eq!(receipt.oracle.backend, provenance.backend);

    // The oracle is built from the preserved TEI bytes, recomputed here rather
    // than trusted: the pins and the receipt agree with the committed files.
    assert_eq!(provenance.fixtures.len(), 3);
    for fixture in &provenance.fixtures {
        assert_eq!(
            fixture.sha256,
            sha256_hex(tei_bytes_for(&fixture.id).as_bytes()),
            "fixture {} does not match its committed bytes",
            fixture.id
        );
        assert!(Path::new(&fixture.path).is_relative());
    }

    // The real provenance clears all four content-band independence requirements.
    let pin = sha256_hex(PROVENANCE.as_bytes());
    require_independent_section_oracle(&generator).expect("the section oracle is independent");
    require_independent_visual_oracle(&NativeVisualOracleProvenance {
        generator_id: generator.clone(),
        backend: receipt.oracle.backend.clone(),
        sha256: pin.clone(),
    })
    .expect("the visual oracle is independent");
    require_independent_callout_oracle(&NativeCalloutOracleProvenance {
        generator_id: generator.clone(),
        backend: receipt.oracle.backend.clone(),
        sha256: pin.clone(),
    })
    .expect("the callout oracle is independent");
    require_independent_body_oracle(&BodyOracleProvenance {
        generator_id: generator.clone(),
        independence: BodyOracleIndependence::Independent,
    })
    .expect("the body oracle is independent");

    // Negative control: a same-generator provenance is refused by every band.
    assert!(
        require_independent_section_oracle(NATIVE_SECTION_SUT_GENERATOR_ID).is_err(),
        "a same-generator section oracle must be refused"
    );
    assert!(
        require_independent_visual_oracle(&NativeVisualOracleProvenance {
            generator_id: NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID.to_owned(),
            backend: receipt.oracle.backend.clone(),
            sha256: pin.clone(),
        })
        .is_err(),
        "a same-generator visual oracle must be refused"
    );
    assert!(
        require_independent_callout_oracle(&NativeCalloutOracleProvenance {
            generator_id: NATIVE_CALLOUT_NATIVE_INPUT_GENERATOR_ID.to_owned(),
            backend: receipt.oracle.backend.clone(),
            sha256: pin.clone(),
        })
        .is_err(),
        "a same-generator callout oracle must be refused"
    );
    assert!(
        require_independent_body_oracle(&BodyOracleProvenance {
            generator_id: NATIVE_BODY_TEXT_GENERATOR_ID.to_owned(),
            independence: BodyOracleIndependence::Independent,
        })
        .is_err(),
        "a same-generator body oracle must be refused"
    );
    assert!(
        require_independent_body_oracle(&BodyOracleProvenance {
            generator_id: generator.clone(),
            independence: BodyOracleIndependence::SameGeneratorAsSut,
        })
        .is_err(),
        "an oracle that declares itself the SUT must be refused"
    );

    // Negative control at the receipt level: the receipt refuses a
    // non-independent oracle and an unexecuted same-generator control.
    let expected = expectation();
    let mut non_independent = receipt.clone();
    non_independent.oracle.independent = false;
    assert_eq!(
        validate_content_receipt(&non_independent, &expected).unwrap_err(),
        ContentReceiptError::OracleNotIndependent
    );
    let mut unrefuted = receipt.clone();
    unrefuted.oracle.same_generator_refused = false;
    assert_eq!(
        validate_content_receipt(&unrefuted, &expected).unwrap_err(),
        ContentReceiptError::OracleIndependenceUnrefuted
    );

    // Negative control: an oracle fixture bound to other bytes is refused.
    let mut wrong_fixture = expectation();
    wrong_fixture.oracle_fixture_sha256.insert(
        normalize_document_identity("hub-2507.19457"),
        sha256_hex(b"not the preserved TEI"),
    );
    assert_eq!(
        validate_content_receipt(&receipt, &wrong_fixture).unwrap_err(),
        ContentReceiptError::DocumentShaDrift {
            document: "hub-2507.19457".to_owned(),
            expected: sha256_hex(b"not the preserved TEI"),
            found: sha256_hex(HUB_TEI.as_bytes()),
        }
    );
}

// ─────────────────────────── named negative mutations ───────────────────────────

#[test]
fn every_receipt_mutation_fails_by_name() {
    let committed = receipt();
    let expected = expectation();

    // A substituted population: the same documents under another stratum label.
    let mut substituted = committed.clone();
    substituted.populations[0].population = ContentPopulationId::ScannedImage;
    assert_eq!(
        validate_content_receipt(&substituted, &expected).unwrap_err(),
        ContentReceiptError::PoolingClaimed {
            field: "populations[scanned_image]".to_owned()
        }
    );

    // A truncated group partition: one declared document loses its assignment.
    let mut truncated = committed.clone();
    truncated.partition.assignments.pop();
    assert_eq!(
        validate_content_receipt(&truncated, &expected).unwrap_err(),
        ContentReceiptError::BindingDrift {
            field: "partition".to_owned(),
            expected: "the recomputed total group partition".to_owned(),
            found: "a drifted partition".to_owned(),
        }
    );

    // A stale declaration pin: the receipt describes an older tree.
    let mut stale = committed.clone();
    stale.declaration.revision_sha256 = sha256_prefixed(b"stale declaration");
    assert_eq!(
        validate_content_receipt(&stale, &expected).unwrap_err(),
        ContentReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: sha256_prefixed(LAYOUT.as_bytes()),
            found: sha256_prefixed(b"stale declaration"),
        }
    );

    // A threshold claim.
    let mut threshold = committed.clone();
    threshold.thresholds = "accepted".to_owned();
    assert_eq!(
        validate_content_receipt(&threshold, &expected).unwrap_err(),
        ContentReceiptError::ThresholdClaimed {
            found: "accepted".to_owned()
        }
    );

    // A product-coverage claim.
    let mut product = committed.clone();
    product.product_coverage_effect = "full".to_owned();
    assert_eq!(
        validate_content_receipt(&product, &expected).unwrap_err(),
        ContentReceiptError::ProductCoverageClaimed {
            found: "full".to_owned()
        }
    );

    // A required family silently moved out of scope.
    let mut out_of_scope = committed.clone();
    let cell = &mut out_of_scope.populations[0].matrix[0].families[3];
    assert_eq!(cell.family.token(), "callout_marker");
    cell.disposition = FORBIDDEN_REQUIRED_FAMILY_TOKEN.to_owned();
    cell.unmet_reason = None;
    cell.observed_count = None;
    assert_eq!(
        validate_content_receipt(&out_of_scope, &expected).unwrap_err(),
        ContentReceiptError::RequiredFamilyOutOfScope {
            token: FORBIDDEN_REQUIRED_FAMILY_TOKEN.to_owned()
        }
    );

    // A vacuous oracle: no required family carries a positive row anywhere.
    let mut vacuous = committed.clone();
    for population in &mut vacuous.populations {
        for document in &mut population.matrix {
            for cell in &mut document.families {
                cell.oracle_positive_rows = 0;
            }
        }
    }
    assert_eq!(
        validate_content_receipt(&vacuous, &expected).unwrap_err(),
        ContentReceiptError::OracleVacuous {
            family: "figure".to_owned()
        }
    );

    // A non-independent oracle.
    let mut not_independent = committed.clone();
    not_independent.oracle.independent = false;
    assert_eq!(
        validate_content_receipt(&not_independent, &expected).unwrap_err(),
        ContentReceiptError::OracleNotIndependent
    );

    // The committed artifact is untouched: every mutation was an in-memory clone,
    // and the tracked bytes still canonicalize.
    assert_eq!(
        std::str::from_utf8(
            &receipt()
                .artifact_json_bytes()
                .expect("the committed receipt still serializes")
        )
        .expect("artifact is UTF-8"),
        RECEIPT_ARTIFACT
    );
}

// ─────────────────────────── hermeticity ───────────────────────────

#[test]
fn the_audit_reads_no_gitignored_corpus() {
    // Every corpus path the receipt carries is a pin, not an input: the audit
    // resolves none of them, and all of its own inputs are git-tracked DATA.
    // This is the S03/T05 lesson — a gate that reads `data/sources/**/*.pdf`
    // does not run from a clean checkout.
    let tracked_inputs = [
        "artifacts/parse_compat/native_content_receipt.json",
        "artifacts/parse_compat/document_segmentation_oracle_source.json",
        "data/parse/layout_bakeoff.yaml",
        "data/parse/native_visual_heads.yaml",
        "data/parse/document_segmentation.yaml",
        "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
        "crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml",
        "crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml",
        "crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
        "scripts/governor/checks/grobid_absorption_progress.yaml",
    ];
    for path in tracked_inputs {
        assert!(
            Path::new(path).is_relative(),
            "`{path}` must be repo-relative"
        );
        assert!(
            !path.starts_with("data/sources/"),
            "the audit must not read the gitignored PDF corpus"
        );
    }

    // The corpus pins are repository-relative and stay inside `data/sources/`;
    // they are evidence of what the producer read, and the audit never opens them.
    let receipt = receipt();
    for population in &receipt.populations {
        for document in &population.documents {
            assert!(document.source_pdf.starts_with("data/sources/"));
            assert!(Path::new(&document.source_pdf).is_relative());
        }
    }
    for document in &receipt.oracle.documents {
        assert!(Path::new(&document.fixture_path).is_relative());
        assert!(!document.fixture_path.starts_with("data/sources/"));
    }

    // The authority locks are carried, not relaxed.
    assert_eq!(
        receipt.locks.get("deployed").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        receipt.locks.get("import_eligible").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        receipt.locks.get("auto_promote").map(String::as_str),
        Some("forbidden")
    );
    assert_eq!(
        receipt.locks.get("graph_write").map(String::as_str),
        Some("forbidden")
    );
    assert_eq!(
        receipt.locks.get("production_route").map(String::as_str),
        Some("forbidden")
    );
}
