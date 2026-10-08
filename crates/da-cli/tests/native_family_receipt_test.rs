//! S04/T07 (M514-1lss2s) — hermetic audit of the two native family receipts.
//!
//! S04/T04 and S04/T05 emit two corpus-bound receipts
//! (`native_family_parity.json`, `native_exclusion_recovery.json`). A receipt is
//! a self-declaration until something else re-reads it, so this file is that
//! independent read: it rebuilds the expectation side from the committed DATA
//! and compares it to the committed artifacts.
//!
//! It is gated on `pdf-layout` because the receipts it audits are produced by
//! `pdf-layout`-only runners: producer and audit ride one feature lane, and a
//! default `cargo test --tests` never builds the producers. The gate executes it
//! in the `da-cli feature-gated example contracts` step.
//!
//! The audit reads only git-tracked DATA (the S03 declaration, the corpus pin,
//! the preserved-TEI provenance, the frozen census carriers and the governor
//! check block), the two git-tracked artifacts, and the real `da-parse`
//! transport seam. It never reads the gitignored `data/sources/**/*.pdf` corpus —
//! a clean checkout does not have it — and never writes the committed artifacts:
//! every mutation below is an in-memory clone.
//!
//! Disciplines proven, each with its own negative:
//!
//! 1. **Replay from the carriers, not from the receipt.** Carrier pins, the
//!    declared corpus, the oracle fixture hashes, the 610/531/79 census and the
//!    72/5/2 split are recomputed from the committed bytes and compared to both
//!    artifacts; the typed receipts must canonicalize back to the committed
//!    bytes.
//! 2. **The governor rule and the receipts agree.** The `native_family_receipt`
//!    block is re-read from the check DATA and must name the same carriers and
//!    the same populations the artifacts carry, and its closed label vocabulary
//!    stays nine entries.
//! 3. **A counted zero needs a guard.** The four in-slice families read
//!    `Observed(0)` only under an installed, identity-isolated guard; the same
//!    receipt without it is refused. The counter is non-vacuous: a real
//!    `GrobidHttpBackend::fetch_tei` on an unreachable `http://` endpoint yields
//!    `Observed(> 0)`, which the receipt refuses (`ObservedJavaOnSlice`).
//! 4. **The oracle is independent.** It is built from preserved TEI bytes
//!    (sha256-pinned in `provenance.yaml`), its generator differs from both SUT
//!    bands, and a same-generator provenance is refused.
//! 5. **The 79 stay the full denominator.** Every frozen exception document is
//!    present exactly once and no document is lost.
//! 6. **Mutations fail by name.** A substituted population, a truncated
//!    denominator, a stale pin, a threshold claim, a product-coverage claim and
//!    a non-independent oracle each produce their own typed refusal.
//!
//! Determinism (ADR-051): no network, no clock, no retry. The single transport
//! call uses a malformed `http://` base URL — it fails at URL parsing, so there
//! is no socket, no DNS and no bind/drop race.

#![cfg(feature = "pdf-layout")]

use da_cli::java_free_slice::load_declaration;
use da_cli::native_family_receipt::{
    AffiliationReference, CLAIM_SCOPE_DIAGNOSTIC_ONLY, CORPUS_POINTER, DECLARATION_MANIFEST_REL,
    DECLARATION_POINTER, DEFAULT_ARTIFACT_REL, DeclaredCorpus, DeclaredDocument,
    EXCLUSION_BASELINE_POINTER, EXCLUSION_BASELINE_REL, EXCLUSION_PROBE_POINTER,
    EXCLUSION_PROBE_REL, EXCLUSION_REASON_ORDER, EXCLUSION_RECOVERY_ARTIFACT_REL,
    ExclusionExpectation, HOLDOUT_MANIFEST_REL, LEAK_MARKERS,
    NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION, NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION,
    NativeExclusionRecoveryReceipt, NativeFamilyReceipt, NativeFamilyReceiptError,
    ORACLE_BACKEND_ID, ORACLE_GENERATOR_ID, ORACLE_MANIFEST_REL, PRODUCT_COVERAGE_EFFECT_NONE,
    ParityExpectation, PopulationId, PopulationKind, REQUIRED_LOCKS, ReceiptKind, RecoveryMode,
    RecoveryPopulation, Redaction, THRESHOLDS_NOT_ACCEPTED, assert_redacted,
    normalize_document_identity, sha256_hex, sha256_prefixed, validate_exclusion_recovery_receipt,
    validate_parity_receipt,
};
use da_parse::{
    BibliographyOracleProvenance, FailureCode, GrobidHttpBackend, GrobidProfile,
    HeaderOracleProvenance, JavaInvocationCounter, JavaInvocationObservation,
    NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID, NATIVE_HEADER_INPUT_GENERATOR_ID, ParseBackend,
    ParseBackendRequest, require_independent_bibliography_oracle,
    require_independent_header_oracle,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ─────────────────────────── committed carriers ───────────────────────────

/// The parity receipt under audit, as committed.
const PARITY_ARTIFACT: &str =
    include_str!("../../../artifacts/parse_compat/native_family_parity.json");
/// The exclusion-recovery receipt under audit, as committed.
const EXCLUSION_ARTIFACT: &str =
    include_str!("../../../artifacts/parse_compat/native_exclusion_recovery.json");
/// The S03 slice declaration and corpus pin.
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
/// The cascade stage carrier the declaration resolves names against.
const COMPATIBILITY: &str = include_str!("../../../data/parse/grobid_compatibility.yaml");
/// The preserved-TEI oracle pins.
const PROVENANCE: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/provenance.yaml");
/// The three preserved-TEI fixtures, as committed.
const HUB_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml");
const RLM_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml");
const RECA_TEI: &str =
    include_str!("../../../crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml");
/// The frozen exclusion carriers.
const HOLDOUT: &str = include_str!("../../../data/parse/holdout_manifests.yaml");
const PROBE: &str =
    include_str!("../../../artifacts/parse_compat/external_holdout_release_probe.json");
const BASELINE: &str =
    include_str!("../../../artifacts/parse_compat/absorption_progress_baseline.json");
/// The governor check block that governs the two receipts.
const GOV_CHECK: &str =
    include_str!("../../../scripts/governor/checks/grobid_absorption_progress.yaml");
/// The frozen acceptance manifest that pins the holdout reference TEI, the
/// independent oracle for the added route documents.
const ACCEPTANCE_MANIFEST: &str = include_str!("../../../data/parse/acceptance_manifest.yaml");
/// The repository-relative path of the acceptance manifest carrier.
const ACCEPTANCE_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
/// The pointer under which that carrier pins the holdout reference TEI: the
/// added route documents' independent oracle, extending the preserved-provenance
/// pins.
const HOLDOUT_REFERENCE_TEI_POINTER: &str = "external_holdout_contract.release_probe.reference_tei";

// ─────────────────────────── DATA mirrors ───────────────────────────

#[derive(Debug, Deserialize)]
struct Provenance {
    backend: String,
    fixtures: Vec<ProvenanceFixture>,
}

#[derive(Debug, Deserialize)]
struct ProvenanceFixture {
    id: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct AcceptanceReferenceTei {
    article_id: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct AcceptanceReleaseProbe {
    reference_tei: Vec<AcceptanceReferenceTei>,
}

#[derive(Debug, Deserialize)]
struct AcceptanceExternalHoldout {
    release_probe: AcceptanceReleaseProbe,
}

#[derive(Debug, Deserialize)]
struct AcceptanceManifest {
    external_holdout_contract: AcceptanceExternalHoldout,
}

#[derive(Debug, Deserialize)]
struct ProbePopulation {
    documents: u64,
    measurable_documents: u64,
    excluded_documents: u64,
    identity_digest_is_byte_hash: bool,
    source_manifest_sha256: String,
    exceptions: Vec<ProbeException>,
}

#[derive(Debug, Deserialize)]
struct ProbeException {
    document_id: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct BaselineDenominator {
    assigned: u64,
    excluded: u64,
    exclusion_reasons: BTreeMap<String, u64>,
}

// ─────────────────────────── helpers ───────────────────────────

fn declaration() -> da_cli::java_free_slice::SliceDeclaration {
    load_declaration(LAYOUT, COMPATIBILITY).expect("the committed slice declaration must load")
}

fn parity_receipt() -> NativeFamilyReceipt {
    NativeFamilyReceipt::parse_json_bytes(PARITY_ARTIFACT.as_bytes())
        .expect("the committed parity receipt must parse")
}

fn exclusion_receipt() -> NativeExclusionRecoveryReceipt {
    NativeExclusionRecoveryReceipt::parse_json_bytes(EXCLUSION_ARTIFACT.as_bytes())
        .expect("the committed exclusion receipt must parse")
}

fn provenance() -> Provenance {
    serde_yaml::from_str(PROVENANCE).expect("the committed provenance must parse")
}

/// Repository root, resolved from this test's own crate directory
/// (`crates/da-cli` → `../..`).
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The frozen holdout reference-TEI rows, as the acceptance manifest pins them.
fn acceptance_reference_tei() -> Vec<AcceptanceReferenceTei> {
    serde_yaml::from_str::<AcceptanceManifest>(ACCEPTANCE_MANIFEST)
        .expect("the committed acceptance manifest must parse")
        .external_holdout_contract
        .release_probe
        .reference_tei
}

/// The pinned reference TEI for a declared document the preserved-provenance
/// carrier does not name, keyed by normalized document identity.
fn reference_tei_path(document_id: &str) -> String {
    let key = normalize_document_identity(document_id);
    acceptance_reference_tei()
        .into_iter()
        .find(|row| normalize_document_identity(&row.article_id) == key)
        .unwrap_or_else(|| panic!("no pinned reference TEI for `{document_id}`"))
        .path
}

/// The pinned oracle fixtures (preserved provenance plus the frozen holdout
/// reference TEI), keyed by normalized identity, as `(path, bare sha256)`.
fn oracle_pins() -> BTreeMap<String, (String, String)> {
    let mut pins = BTreeMap::new();
    for fixture in provenance().fixtures {
        pins.insert(
            normalize_document_identity(&fixture.id),
            (fixture.path, fixture.sha256),
        );
    }
    for row in acceptance_reference_tei() {
        let sha = row
            .sha256
            .strip_prefix("sha256:")
            .unwrap_or(&row.sha256)
            .to_owned();
        pins.insert(
            normalize_document_identity(&row.article_id),
            (row.path, sha),
        );
    }
    pins
}

/// Verify the receipt's added oracle documents are exactly the bytes the
/// acceptance-manifest `reference_tei` carrier pins. The carrier extends the
/// preserved-provenance oracle over the declared corpus documents the preserved
/// fixtures do not cover, so those documents must be pinned here by normalized
/// identity, path and digest -- never trusted from the receipt. A carrier the
/// receipt reads no bytes from is refused rather than accepted as decorative.
/// Returns the first disagreement, or `None` when the carrier is bound.
fn holdout_reference_tei_binding_error(
    manifest: &str,
    receipt: &NativeFamilyReceipt,
) -> Option<String> {
    let rows = match serde_yaml::from_str::<AcceptanceManifest>(manifest) {
        Ok(parsed) => parsed.external_holdout_contract.release_probe.reference_tei,
        Err(error) => return Some(format!("the acceptance manifest does not parse: {error}")),
    };
    let pins: BTreeMap<String, &AcceptanceReferenceTei> = rows
        .iter()
        .map(|row| (normalize_document_identity(&row.article_id), row))
        .collect();
    let preserved: BTreeSet<String> = provenance()
        .fixtures
        .iter()
        .map(|fixture| normalize_document_identity(&fixture.id))
        .collect();
    let mut covered = 0usize;
    for document in &receipt.oracle.documents {
        let key = normalize_document_identity(&document.document_id);
        if preserved.contains(&key) {
            continue;
        }
        let pin = match pins.get(&key) {
            Some(pin) => pin,
            None => {
                return Some(format!(
                    "the receipt oracle document `{}` is not pinned by the holdout reference TEI carrier",
                    document.document_id
                ));
            }
        };
        if document.fixture_path != pin.path {
            return Some(format!(
                "the receipt fixture path `{}` for `{}` disagrees with the acceptance manifest pin `{}`",
                document.fixture_path, document.document_id, pin.path
            ));
        }
        let pin_sha256 = pin.sha256.strip_prefix("sha256:").unwrap_or(&pin.sha256);
        if document.sha256 != pin_sha256 {
            return Some(format!(
                "the receipt fixture digest for `{}` disagrees with the acceptance manifest pin",
                document.document_id
            ));
        }
        covered += 1;
    }
    if covered == 0 {
        return Some(
            "the receipt carries no added oracle document, so the holdout reference TEI carrier is unread".to_owned(),
        );
    }
    None
}

fn probe_population() -> ProbePopulation {
    let value: serde_json::Value = serde_json::from_str(PROBE).expect("the probe must parse");
    serde_json::from_value(
        value
            .get("reference_population")
            .expect("the probe carries no reference_population")
            .clone(),
    )
    .expect("the probe reference_population must parse")
}

fn baseline_denominator() -> BaselineDenominator {
    let value: serde_json::Value = serde_json::from_str(BASELINE).expect("the baseline must parse");
    serde_json::from_value(
        value
            .get("denominator")
            .expect("the baseline carries no denominator")
            .clone(),
    )
    .expect("the baseline denominator must parse")
}

fn tei_bytes_for(document_id: &str) -> String {
    // Both id spellings (`hub_2507.19457` from the provenance pins and
    // `hub-2507.19457` from the corpus declaration) normalize to the same key,
    // so the arXiv id substring identifies the fixture.
    if document_id.contains("2507.19457") {
        HUB_TEI.to_owned()
    } else if document_id.contains("2512.24601") {
        RLM_TEI.to_owned()
    } else if document_id.contains("2605.26525") {
        RECA_TEI.to_owned()
    } else {
        // The added route documents are pinned by the frozen holdout reference
        // TEI in the acceptance manifest. Read the git-tracked fixture bytes and
        // let the caller hash them; a document with no pinned reference TEI is
        // refused by name.
        let path = reference_tei_path(document_id);
        std::fs::read_to_string(repo_root().join(&path)).unwrap_or_else(|error| {
            panic!("cannot read holdout TEI `{path}` for `{document_id}`: {error}")
        })
    }
}

/// The declared corpus, rebuilt from the committed declaration bytes.
fn parity_expectation() -> ParityExpectation {
    let declaration = declaration();
    let corpus = DeclaredCorpus {
        manifest: DECLARATION_MANIFEST_REL.to_owned(),
        pointer: CORPUS_POINTER.to_owned(),
        revision_sha256: sha256_prefixed(LAYOUT.as_bytes()),
        corpus_id: declaration.corpus_id.clone(),
        document_count: declaration.corpus_n as u64,
        page_denominator: declaration.page_denominator,
        documents: declaration
            .documents
            .iter()
            .map(|document| DeclaredDocument {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            })
            .collect(),
    };
    let mut oracle_fixture_sha256 = BTreeMap::new();
    for key in oracle_pins().into_keys() {
        oracle_fixture_sha256.insert(key.clone(), sha256_hex(tei_bytes_for(&key).as_bytes()));
    }
    ParityExpectation {
        corpus,
        oracle_revision_sha256: sha256_prefixed(PROVENANCE.as_bytes()),
        oracle_fixture_sha256,
    }
}

/// The frozen census and typed exception identities, rebuilt from the committed
/// probe and baseline bytes.
fn exclusion_expectation() -> ExclusionExpectation {
    let probe = probe_population();
    let baseline = baseline_denominator();
    let population = RecoveryPopulation {
        population_id: PopulationId::ExclusionBaseline,
        population_kind: PopulationKind::Exclusion,
        mode: RecoveryMode::Full,
        assumed_complete: true,
        assigned: baseline.assigned,
        excluded: baseline.excluded,
        measurable: probe.measurable_documents,
        targeted: baseline.excluded,
        reasons: baseline.exclusion_reasons.clone(),
    };
    ExclusionExpectation {
        declaration_revision_sha256: sha256_prefixed(HOLDOUT.as_bytes()),
        probe_revision_sha256: sha256_prefixed(PROBE.as_bytes()),
        baseline_revision_sha256: sha256_prefixed(BASELINE.as_bytes()),
        population,
        affiliation_reference: affiliation_reference_from_baseline(),
        documents: probe
            .exceptions
            .iter()
            .map(|exception| exception.document_id.clone())
            .collect(),
    }
}

fn affiliation_reference_from_baseline() -> AffiliationReference {
    let value: serde_json::Value = serde_json::from_str(BASELINE).expect("the baseline must parse");
    let measurements = value
        .get("measurements")
        .and_then(serde_json::Value::as_array)
        .expect("the baseline carries no measurements");
    for measurement in measurements {
        let per_family = measurement
            .get("strata")
            .and_then(|strata| strata.get("per_family"));
        let recovered = per_family
            .and_then(|family| family.get("numerator"))
            .and_then(serde_json::Value::as_u64);
        let denominator = per_family
            .and_then(|family| family.get("denominator"))
            .and_then(serde_json::Value::as_u64);
        if let (Some(recovered), Some(denominator)) = (recovered, denominator) {
            let metric_id = measurement
                .get("metric_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !metric_id.is_empty() && denominator > 0 {
                return AffiliationReference {
                    metric_id: metric_id.to_owned(),
                    recovered,
                    denominator,
                };
            }
        }
    }
    panic!("the baseline carries no per-family capability reference");
}

fn gov_block() -> serde_yaml::Value {
    let document: serde_yaml::Value =
        serde_yaml::from_str(GOV_CHECK).expect("the check must parse");
    document
        .get("native_family_receipt")
        .expect("the check carries no native_family_receipt block")
        .clone()
}

fn yaml_at<'a>(value: &'a serde_yaml::Value, path: &[&str]) -> &'a serde_yaml::Value {
    let mut current = value;
    for key in path {
        current = current
            .get(*key)
            .unwrap_or_else(|| panic!("governor check is missing `{path:?}`"));
    }
    current
}

// ─────────────────────────── replay: parity receipt ───────────────────────────

#[test]
fn committed_parity_receipt_replays_from_the_committed_carriers() {
    let declaration = declaration();
    let receipt = parity_receipt();
    let expectation = parity_expectation();

    // The runner's default artifact path is the committed artifact.
    assert_eq!(
        DEFAULT_ARTIFACT_REL,
        "artifacts/parse_compat/native_family_parity.json"
    );
    assert_eq!(receipt.schema_version, NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION);
    assert_eq!(receipt.receipt_kind, ReceiptKind::ParityThreeDocumentSlice);
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);

    // The S03 declaration pointer the receipt is bound to, re-derived from the
    // declaration bytes.
    assert_eq!(declaration.declaration_manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(declaration.declaration_pointer, DECLARATION_POINTER);
    assert_eq!(declaration.corpus_manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(declaration.corpus_pointer, CORPUS_POINTER);
    assert_eq!(receipt.declaration.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, declaration.declaration_pointer);
    assert_eq!(
        receipt.declaration.revision_sha256,
        sha256_prefixed(LAYOUT.as_bytes()),
        "the declaration pin must be the committed declaration bytes"
    );

    // The corpus population, pinned to the same bytes it is declared in.
    assert_eq!(receipt.populations.len(), 1);
    let population = &receipt.populations[0];
    assert_eq!(population.population_id, PopulationId::ThreeDocumentSlice);
    assert_eq!(population.population_kind, PopulationKind::Corpus);
    assert_eq!(population.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(population.pointer, CORPUS_POINTER);
    assert_eq!(
        population.revision_sha256,
        sha256_prefixed(LAYOUT.as_bytes())
    );
    assert_eq!(population.document_count, declaration.corpus_n as u64);
    assert_eq!(population.page_denominator, declaration.page_denominator);
    assert_eq!(population.page_denominator, 381);
    assert_eq!(population.documents.len(), declaration.documents.len());
    for (identity, declared) in population.documents.iter().zip(&declaration.documents) {
        assert_eq!(identity.id, declared.id);
        assert_eq!(identity.source_pdf, declared.source_pdf);
        assert_eq!(identity.sha256, declared.sha256);
        assert_eq!(identity.pages, declared.pages);
        assert!(Path::new(&identity.source_pdf).is_relative());
        assert!(identity.source_pdf.starts_with("data/sources/"));
    }
    assert_eq!(population.matrix.len(), declaration.documents.len());
    for (row, declared) in population.matrix.iter().zip(&declaration.documents) {
        assert_eq!(row.id, declared.id);
        assert_eq!(row.sha256, declared.sha256);
        assert_eq!(row.pages, declared.pages);
        assert_eq!(row.observed_pages, declared.pages);
        // Every family and region exactly once, in canonical order.
        assert_eq!(row.families.len(), 7);
        assert_eq!(row.regions.len(), 2);
        for cell in &row.families {
            match cell.disposition {
                da_cli::native_family_receipt::FamilyDisposition::Observed => {
                    assert!(
                        cell.observed_count.is_some(),
                        "{} {} is Observed without a count",
                        row.id,
                        cell.family.as_str()
                    );
                    assert!(cell.refusal.is_none());
                }
                _ => assert!(cell.observed_count.is_none()),
            }
        }
        for region in &row.regions {
            assert!(region.missing_char_count <= region.oracle_char_count);
            let expected = if region.oracle_char_count == 0 {
                da_cli::native_family_receipt::CoverageVerdict::NotAssessable
            } else if region.missing_char_count == 0 {
                da_cli::native_family_receipt::CoverageVerdict::GranularityOnly
            } else {
                da_cli::native_family_receipt::CoverageVerdict::ContentLoss
            };
            assert_eq!(region.verdict, expected);
        }
    }

    // The oracle: identity, independence flags, and fixture bytes recomputed
    // from the preserved TEI rather than trusted from the receipt.
    assert_eq!(receipt.oracle.manifest, ORACLE_MANIFEST_REL);
    assert_eq!(
        receipt.oracle.revision_sha256,
        sha256_prefixed(PROVENANCE.as_bytes())
    );
    assert_eq!(receipt.oracle.generator_id.as_str(), ORACLE_GENERATOR_ID);
    assert_eq!(receipt.oracle.backend.as_str(), ORACLE_BACKEND_ID);
    assert!(receipt.oracle.independent);
    assert!(receipt.oracle.same_generator_refused);
    assert_eq!(receipt.oracle.documents.len(), declaration.documents.len());
    for fixture in &receipt.oracle.documents {
        let expected_sha = expectation
            .oracle_fixture_sha256
            .get(&normalize_document_identity(&fixture.document_id))
            .expect("the receipt names only pinned oracle documents");
        assert_eq!(&fixture.sha256, expected_sha);
        assert_eq!(
            fixture.sha256,
            sha256_hex(tei_bytes_for(&fixture.document_id).as_bytes())
        );
        assert!(Path::new(&fixture.fixture_path).is_relative());
    }

    // The guard: the counted zero is a measurement under an installed,
    // identity-isolated observer on the real Java seam.
    assert_eq!(receipt.guard.observer.as_str(), "java_transport_fetch_tei");
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(receipt.guard.observed_java_calls, 0);

    // Locks, R128 separation and redaction.
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(receipt.locks.get(lock).map(String::as_str), Some(value));
    }
    assert_eq!(receipt.locks.len(), REQUIRED_LOCKS.len());
    assert!(!receipt.pooling.citation_names_pooled_with_biblstruct);
    assert_eq!(receipt.redaction, Redaction::clean());

    // Byte-for-byte canonical replay, and the redaction invariant on the bytes.
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        PARITY_ARTIFACT,
        "the committed artifact must be exactly the canonical serialization"
    );
    assert_redacted(PARITY_ARTIFACT).expect("the committed artifact is redacted");
    for marker in LEAK_MARKERS {
        assert!(
            !PARITY_ARTIFACT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }

    // And the whole thing re-reads against the committed sources.
    validate_parity_receipt(&receipt, &expectation)
        .expect("the committed receipt must re-validate against the committed carriers");
}

// ─────────────────────────── replay: governor rule ───────────────────────────

#[test]
fn the_governor_rule_and_the_receipts_agree_on_the_committed_data() {
    let block = gov_block();
    let receipt = parity_receipt();
    let exclusion = exclusion_receipt();

    // The rule's declaration pointer is the S03 declaration the receipt binds.
    assert_eq!(
        yaml_at(&block, &["declaration", "manifest"]).as_str(),
        Some(DECLARATION_MANIFEST_REL)
    );
    assert_eq!(
        yaml_at(&block, &["declaration", "pointer"]).as_str(),
        Some(DECLARATION_POINTER)
    );
    assert_eq!(receipt.declaration.manifest, DECLARATION_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, DECLARATION_POINTER);

    // The rule's receipt paths are the artifacts this audit re-reads.
    assert_eq!(
        yaml_at(&block, &["receipts", "parity", "path"]).as_str(),
        Some(DEFAULT_ARTIFACT_REL)
    );
    assert_eq!(
        yaml_at(&block, &["receipts", "exclusion", "path"]).as_str(),
        Some(EXCLUSION_RECOVERY_ARTIFACT_REL)
    );
    assert_eq!(
        yaml_at(&block, &["receipts", "parity", "schema_version"]).as_str(),
        Some(NATIVE_FAMILY_RECEIPT_SCHEMA_VERSION)
    );
    assert_eq!(
        yaml_at(&block, &["receipts", "exclusion", "schema_version"]).as_str(),
        Some(NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION)
    );

    // Every carrier the rule names must be a pin one of the receipts carries.
    // The holdout reference TEI carrier extends the preserved-provenance oracle
    // over the added route documents, so it is admitted only once the receipt's
    // added oracle documents are proven to be exactly the bytes the acceptance
    // manifest pins.
    let holdout_binding = holdout_reference_tei_binding_error(ACCEPTANCE_MANIFEST, &receipt);
    assert_eq!(
        holdout_binding, None,
        "the holdout reference TEI carrier does not bind the receipt: {holdout_binding:?}"
    );
    let carriers = yaml_at(&block, &["carriers"])
        .as_sequence()
        .expect("the rule declares carriers");
    assert_eq!(carriers.len(), 6);
    for carrier in carriers {
        let path = carrier
            .get("path")
            .and_then(serde_yaml::Value::as_str)
            .expect("a carrier names a path");
        let pointer = carrier
            .get("pointer")
            .and_then(serde_yaml::Value::as_str)
            .expect("a carrier names a pointer");
        let named = [
            (
                receipt.declaration.manifest.as_str(),
                receipt.declaration.pointer.as_str(),
            ),
            (receipt.oracle.manifest.as_str(), "fixtures"),
            (
                population_of(&receipt).manifest.as_str(),
                population_of(&receipt).pointer.as_str(),
            ),
            (
                exclusion.declaration.manifest.as_str(),
                exclusion.declaration.pointer.as_str(),
            ),
            (
                exclusion.probe.manifest.as_str(),
                exclusion.probe.pointer.as_str(),
            ),
            (
                exclusion.baseline.manifest.as_str(),
                exclusion.baseline.pointer.as_str(),
            ),
            (ACCEPTANCE_MANIFEST_REL, HOLDOUT_REFERENCE_TEI_POINTER),
        ];
        assert!(
            named.iter().any(
                |(candidate_path, candidate_pointer)| *candidate_path == path
                    && *candidate_pointer == pointer
            ),
            "the rule carrier `{path}#{pointer}` is not pinned by either receipt"
        );
    }

    // Populations: the rule's parity and exclusion census equal the artifacts.
    assert_eq!(
        yaml_at(&block, &["populations", "parity", "document_count"]).as_u64(),
        Some(population_of(&receipt).document_count)
    );
    assert_eq!(
        yaml_at(&block, &["populations", "parity", "page_denominator"]).as_u64(),
        Some(population_of(&receipt).page_denominator)
    );
    let expected = exclusion_expectation().population;
    let rule_exclusion = yaml_at(&block, &["populations", "exclusion"]);
    assert_eq!(
        rule_exclusion
            .get("assigned")
            .and_then(serde_yaml::Value::as_u64),
        Some(expected.assigned)
    );
    assert_eq!(
        rule_exclusion
            .get("excluded")
            .and_then(serde_yaml::Value::as_u64),
        Some(expected.excluded)
    );
    assert_eq!(
        rule_exclusion
            .get("measurable")
            .and_then(serde_yaml::Value::as_u64),
        Some(expected.measurable)
    );
    assert_eq!(
        rule_exclusion
            .get("targeted")
            .and_then(serde_yaml::Value::as_u64),
        Some(expected.targeted)
    );
    let rule_reasons: BTreeMap<String, u64> = rule_exclusion
        .get("reason_counts")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("the rule declares reason_counts")
        .iter()
        .map(|(token, count)| {
            (
                token.as_str().expect("a reason token").to_owned(),
                count.as_u64().expect("a reason count"),
            )
        })
        .collect();
    assert_eq!(rule_reasons, expected.reasons);
    assert_eq!(exclusion.population.assigned, expected.assigned);
    assert_eq!(exclusion.population.excluded, expected.excluded);
    assert_eq!(exclusion.population.measurable, expected.measurable);
    assert_eq!(exclusion.population.targeted, expected.targeted);
    assert_eq!(exclusion.population.reasons, expected.reasons);

    // Expected claim values: the rule and the artifacts agree.
    let rule_expected = yaml_at(&block, &["expected"]);
    assert_eq!(
        rule_expected
            .get("claim_scope")
            .and_then(serde_yaml::Value::as_str),
        Some(CLAIM_SCOPE_DIAGNOSTIC_ONLY)
    );
    assert_eq!(
        rule_expected
            .get("product_coverage_effect")
            .and_then(serde_yaml::Value::as_str),
        Some(PRODUCT_COVERAGE_EFFECT_NONE)
    );
    assert_eq!(
        rule_expected
            .get("thresholds")
            .and_then(serde_yaml::Value::as_str),
        Some(THRESHOLDS_NOT_ACCEPTED)
    );
    assert_eq!(
        rule_expected
            .get("guard_observer")
            .and_then(serde_yaml::Value::as_str),
        Some("java_transport_fetch_tei")
    );
    assert_eq!(
        rule_expected
            .get("oracle_independent")
            .and_then(serde_yaml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        rule_expected
            .get("same_generator_refused")
            .and_then(serde_yaml::Value::as_bool),
        Some(true)
    );
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert!(receipt.oracle.independent);
    assert!(receipt.oracle.same_generator_refused);

    // The closed label vocabulary is nine labels, and the negative fixture is
    // required to raise every one of them. This is the rule-side half of the
    // "each mutation fails by name" discipline the Rust half proves for the
    // builder/validator vocabulary.
    let labels = yaml_at(&block, &["violations"])
        .as_sequence()
        .expect("the rule declares a violation vocabulary");
    assert_eq!(
        labels.len(),
        9,
        "the native family label set is closed at nine"
    );
    let required = yaml_at(
        &block,
        &["fixtures", "negative", "expect", "require_violations"],
    )
    .as_sequence()
    .expect("the negative fixture requires violations");
    assert_eq!(required.len(), 9);
    for label in labels {
        assert!(
            required.contains(label),
            "the negative fixture omits {label:?}"
        );
    }
}

#[test]
fn a_holdout_reference_tei_carrier_requires_committed_pins() {
    let receipt = parity_receipt();

    // Positive control: the committed manifest binds the receipt's nine added
    // oracle documents by identity, path and digest.
    assert_eq!(
        holdout_reference_tei_binding_error(ACCEPTANCE_MANIFEST, &receipt),
        None
    );

    // Negative: a substituted reference-TEI digest cannot pass as its own.
    let substituted = ACCEPTANCE_MANIFEST.replace(
        "sha256:11c46c00dc4b97b47c9591356d29eedd888e467d0d042fa104d2583ee6f3bb55",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_ne!(substituted, ACCEPTANCE_MANIFEST);
    let error = holdout_reference_tei_binding_error(&substituted, &receipt)
        .expect("a substituted holdout reference-TEI pin must be refused");
    assert!(
        error.contains("1409.0473"),
        "the refusal must name the substituted document: {error}"
    );

    // Negative: a dropped reference-TEI row leaves the receipt's added document
    // unpinned and is refused.
    let mut manifest: serde_yaml::Value =
        serde_yaml::from_str(ACCEPTANCE_MANIFEST).expect("the acceptance manifest parses");
    manifest["external_holdout_contract"]["release_probe"]["reference_tei"]
        .as_sequence_mut()
        .expect("the acceptance manifest pins reference_tei")
        .retain(|row| {
            row.get("article_id").and_then(serde_yaml::Value::as_str) != Some("1409.0473")
        });
    let dropped = serde_yaml::to_string(&manifest).expect("the mutated manifest serializes");
    let error = holdout_reference_tei_binding_error(&dropped, &receipt)
        .expect("a dropped holdout reference-TEI row must be refused");
    assert!(
        error.contains("1409.0473"),
        "the refusal must name the unpinned document: {error}"
    );

    // Negative: a carrier the receipt reads no bytes from is decorative, not a
    // proof, so it is refused rather than accepted.
    let mut stripped = receipt.clone();
    stripped.oracle.documents.retain(|document| {
        let key = normalize_document_identity(&document.document_id);
        provenance()
            .fixtures
            .iter()
            .any(|fixture| normalize_document_identity(&fixture.id) == key)
    });
    let error = holdout_reference_tei_binding_error(ACCEPTANCE_MANIFEST, &stripped)
        .expect("a carrier the receipt does not read must be refused");
    assert!(
        error.contains("unread"),
        "the refusal must explain the unread carrier: {error}"
    );
}

fn population_of(
    receipt: &NativeFamilyReceipt,
) -> &da_cli::native_family_receipt::PopulationReceipt {
    receipt
        .populations
        .iter()
        .find(|population| population.population_id == PopulationId::ThreeDocumentSlice)
        .expect("the parity population is present")
}

// ─────────────────────────── replay: exclusion receipt ───────────────────────────

#[test]
fn committed_exclusion_receipt_replays_from_the_frozen_census() {
    let receipt = exclusion_receipt();
    let expectation = exclusion_expectation();

    assert_eq!(
        EXCLUSION_RECOVERY_ARTIFACT_REL,
        "artifacts/parse_compat/native_exclusion_recovery.json"
    );
    assert_eq!(
        receipt.schema_version,
        NATIVE_EXCLUSION_RECOVERY_SCHEMA_VERSION
    );
    assert_eq!(receipt.receipt_kind, ReceiptKind::ExclusionRecoveryBaseline);
    assert_eq!(receipt.claim_scope, CLAIM_SCOPE_DIAGNOSTIC_ONLY);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);

    // The three carrier pins, recomputed from the committed bytes.
    assert_eq!(receipt.declaration.manifest, HOLDOUT_MANIFEST_REL);
    assert_eq!(receipt.declaration.pointer, "articles");
    assert_eq!(
        receipt.declaration.revision_sha256,
        sha256_prefixed(HOLDOUT.as_bytes())
    );
    assert_eq!(receipt.probe.manifest, EXCLUSION_PROBE_REL);
    assert_eq!(receipt.probe.pointer, EXCLUSION_PROBE_POINTER);
    assert_eq!(
        receipt.probe.revision_sha256,
        sha256_prefixed(PROBE.as_bytes())
    );
    assert_eq!(receipt.baseline.manifest, EXCLUSION_BASELINE_REL);
    assert_eq!(receipt.baseline.pointer, EXCLUSION_BASELINE_POINTER);
    assert_eq!(
        receipt.baseline.revision_sha256,
        sha256_prefixed(BASELINE.as_bytes())
    );

    // The census: 610 assigned, 79 excluded, 531 measurable, 72/5/2 typed.
    let probe = probe_population();
    let baseline = baseline_denominator();
    assert_eq!(probe.documents, 610);
    assert_eq!(probe.excluded_documents, 79);
    assert_eq!(probe.measurable_documents, 531);
    assert!(!probe.identity_digest_is_byte_hash);
    assert_eq!(
        probe.source_manifest_sha256,
        sha256_prefixed(HOLDOUT.as_bytes())
    );
    assert_eq!(
        probe.documents - probe.excluded_documents,
        probe.measurable_documents
    );
    assert_eq!(baseline.assigned, 610);
    assert_eq!(baseline.excluded, 79);
    assert_eq!(baseline.exclusion_reasons, expectation.population.reasons);
    assert_eq!(baseline.exclusion_reasons.values().sum::<u64>(), 79);
    // The 72/5/2 split is recomputed from the probe's typed exception list too,
    // so it cannot be a restatement of the baseline alone.
    let mut probe_counts: BTreeMap<String, u64> = BTreeMap::new();
    for exception in &probe.exceptions {
        *probe_counts.entry(exception.reason.clone()).or_default() += 1;
    }
    assert_eq!(probe_counts, baseline.exclusion_reasons);
    assert_eq!(receipt.population, expectation.population);
    assert_eq!(receipt.population.mode, RecoveryMode::Full);
    assert!(receipt.population.assumed_complete);

    // The 79 stay the full denominator, and no typed exception document is lost.
    assert_eq!(receipt.recoveries.len(), 79);
    assert_eq!(receipt.population.excluded, 79);
    assert_eq!(receipt.population.targeted, 79);
    let typed: BTreeSet<String> = probe
        .exceptions
        .iter()
        .map(|exception| exception.document_id.clone())
        .collect();
    assert_eq!(typed.len(), 79);
    assert_eq!(typed, expectation.documents);
    let observed: BTreeSet<String> = receipt
        .recoveries
        .iter()
        .map(|row| row.document_id.clone())
        .collect();
    assert_eq!(observed, typed, "no exception document may be lost");
    assert_eq!(
        observed.len(),
        receipt.recoveries.len(),
        "no duplicate document"
    );

    // Aggregates partition the denominator per reason.
    assert_eq!(receipt.aggregates.len(), EXCLUSION_REASON_ORDER.len());
    let mut total = 0u64;
    for reason in EXCLUSION_REASON_ORDER {
        let aggregate = receipt
            .aggregates
            .iter()
            .find(|aggregate| aggregate.reason == reason)
            .expect("every typed reason carries an aggregate");
        let frozen = receipt
            .population
            .reasons
            .get(reason.as_str())
            .copied()
            .expect("the frozen census names every reason");
        assert_eq!(aggregate.denominator, frozen);
        assert_eq!(
            aggregate.denominator,
            aggregate.recovered
                + aggregate.not_recovered
                + aggregate.out_of_scope
                + aggregate.typed_refusal
        );
        // D502: no producer in this cut may claim a field recovered.
        assert_eq!(
            aggregate.recovered, 0,
            "atom recognition is not field attribution"
        );
        total += aggregate.denominator;
    }
    assert_eq!(total, 79);

    // The frozen affiliation capability reference is carried, not absorbed.
    assert_eq!(receipt.affiliation_reference.recovered, 3055);
    assert_eq!(receipt.affiliation_reference.denominator, 7077);
    assert_eq!(
        receipt.affiliation_reference,
        expectation.affiliation_reference
    );

    // Integration failures are typed and, on the committed artifact, empty.
    assert!(receipt.integrity_failures.is_empty());

    // Guard, locks and redaction.
    assert!(receipt.guard.installed);
    assert!(receipt.guard.identity_isolated);
    assert_eq!(receipt.guard.observed_java_calls, 0);
    for (lock, value) in REQUIRED_LOCKS {
        assert_eq!(receipt.locks.get(lock).map(String::as_str), Some(value));
    }
    assert_eq!(receipt.locks.len(), REQUIRED_LOCKS.len());
    assert_eq!(receipt.redaction, Redaction::clean());

    // Byte-for-byte canonical replay and redaction of the bytes.
    let reserialized = receipt
        .artifact_json_bytes()
        .expect("the typed receipt re-serializes");
    assert_eq!(
        std::str::from_utf8(&reserialized).expect("artifact is UTF-8"),
        EXCLUSION_ARTIFACT,
        "the committed artifact must be exactly the canonical serialization"
    );
    assert_redacted(EXCLUSION_ARTIFACT).expect("the committed artifact is redacted");
    for marker in LEAK_MARKERS {
        assert!(
            !EXCLUSION_ARTIFACT.contains(marker),
            "the committed artifact leaked `{marker}`"
        );
    }

    // Every row's source PDF is a repository-relative pin the audit never opens.
    for row in &receipt.recoveries {
        if let Some(source_pdf) = &row.source_pdf {
            assert!(Path::new(source_pdf).is_relative());
            assert!(source_pdf.starts_with("data/sources/"));
        }
    }

    validate_exclusion_recovery_receipt(&receipt, &expectation)
        .expect("the committed receipt must re-validate against the frozen census");
}

// ─────────────────────────── guard provenance ───────────────────────────

#[test]
fn a_counted_zero_requires_an_installed_and_isolated_guard() {
    let expectation = parity_expectation();
    let committed = parity_receipt();

    // Positive: the committed receipt validates under its guard.
    validate_parity_receipt(&committed, &expectation).expect("the committed receipt is guarded");

    // Negative: the very same zero without an installed guard is refused.
    let mut unguarded = committed.clone();
    unguarded.guard.installed = false;
    assert_eq!(
        validate_parity_receipt(&unguarded, &expectation).unwrap_err(),
        NativeFamilyReceiptError::GuardNotInstalled
    );

    // Negative: a guard that rotates the cache identity cannot attribute its zero.
    let mut rotated = committed.clone();
    rotated.guard.identity_isolated = false;
    assert_eq!(
        validate_parity_receipt(&rotated, &expectation).unwrap_err(),
        NativeFamilyReceiptError::GuardIdentityNotIsolated
    );

    // Negative: a measured Java consult is never a zero.
    let mut consulted = committed.clone();
    consulted.guard.observed_java_calls = 1;
    assert_eq!(
        validate_parity_receipt(&consulted, &expectation).unwrap_err(),
        NativeFamilyReceiptError::ObservedJavaOnSlice { attempts: 1 }
    );

    // Negative: the same rule holds for the exclusion receipt.
    let exclusion = exclusion_receipt();
    let mut exclusion_unguarded = exclusion.clone();
    exclusion_unguarded.guard.installed = false;
    assert_eq!(
        validate_exclusion_recovery_receipt(&exclusion_unguarded, &exclusion_expectation())
            .unwrap_err(),
        NativeFamilyReceiptError::GuardNotInstalled
    );
    let mut exclusion_consulted = exclusion;
    exclusion_consulted.guard.observed_java_calls = 1;
    assert_eq!(
        validate_exclusion_recovery_receipt(&exclusion_consulted, &exclusion_expectation())
            .unwrap_err(),
        NativeFamilyReceiptError::ObservedJavaOnSlice { attempts: 1 }
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
            "m514-s04-t07",
            b"%PDF-1.4 native-family-receipt fixture".to_vec(),
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
    let mut receipt = parity_receipt();
    receipt.guard.observed_java_calls = attempts;
    assert_eq!(
        validate_parity_receipt(&receipt, &parity_expectation()).unwrap_err(),
        NativeFamilyReceiptError::ObservedJavaOnSlice { attempts }
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
    let receipt = parity_receipt();

    // The oracle generator is not either native band, and its backend is the
    // other generator that produced the preserved bytes.
    assert_ne!(ORACLE_GENERATOR_ID, NATIVE_HEADER_INPUT_GENERATOR_ID);
    assert_ne!(ORACLE_GENERATOR_ID, NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID);
    let provenance = provenance();
    assert_eq!(provenance.backend, ORACLE_BACKEND_ID);
    assert_eq!(receipt.oracle.generator_id.as_str(), ORACLE_GENERATOR_ID);
    assert_eq!(receipt.oracle.backend.as_str(), provenance.backend);

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

    // The real provenance clears both independence requirements.
    let sha = sha256_hex(PROVENANCE.as_bytes());
    let header = HeaderOracleProvenance {
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: provenance.backend.clone(),
        sha256: sha.clone(),
    };
    let bibliography = BibliographyOracleProvenance {
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: provenance.backend.clone(),
        sha256: sha.clone(),
    };
    require_independent_header_oracle(&header).expect("the oracle is independent");
    require_independent_bibliography_oracle(&bibliography).expect("the oracle is independent");

    // Negative control: a same-generator provenance is refused by both bands.
    assert!(
        require_independent_header_oracle(&HeaderOracleProvenance {
            generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.to_owned(),
            backend: provenance.backend.clone(),
            sha256: sha.clone(),
        })
        .is_err(),
        "a same-generator header oracle must be refused"
    );
    assert!(
        require_independent_bibliography_oracle(&BibliographyOracleProvenance {
            generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.to_owned(),
            backend: provenance.backend,
            sha256: sha,
        })
        .is_err(),
        "a same-generator bibliography oracle must be refused"
    );

    // Negative control at the receipt level: the receipt refuses a
    // non-independent oracle and an unexecuted same-generator control.
    let expectation = parity_expectation();
    let mut non_independent = parity_receipt();
    non_independent.oracle.independent = false;
    assert_eq!(
        validate_parity_receipt(&non_independent, &expectation).unwrap_err(),
        NativeFamilyReceiptError::OracleNotIndependent
    );
    let mut unrefuted = parity_receipt();
    unrefuted.oracle.same_generator_refused = false;
    assert_eq!(
        validate_parity_receipt(&unrefuted, &expectation).unwrap_err(),
        NativeFamilyReceiptError::OracleIndependenceUnrefuted
    );

    // Negative control: an oracle fixture bound to other bytes is refused.
    let mut wrong_fixture = parity_expectation();
    wrong_fixture.oracle_fixture_sha256.insert(
        normalize_document_identity("hub-2507.19457"),
        sha256_hex(b"not the preserved TEI"),
    );
    assert_eq!(
        validate_parity_receipt(&parity_receipt(), &wrong_fixture).unwrap_err(),
        NativeFamilyReceiptError::DocumentShaDrift {
            document: "hub-2507.19457".to_owned(),
            expected: sha256_hex(b"not the preserved TEI"),
            found: sha256_hex(HUB_TEI.as_bytes()),
        }
    );
}

// ─────────────────────────── named negative mutations ───────────────────────────

#[test]
fn every_receipt_mutation_fails_by_name() {
    let parity = parity_receipt();
    let parity_expectation = parity_expectation();
    let exclusion = exclusion_receipt();
    let exclusion_expectation = exclusion_expectation();

    // A substituted population (the sibling population, not the exclusion one).
    let mut substituted = exclusion_expectation.clone();
    substituted.population.population_id = PopulationId::ThreeDocumentSlice;
    assert_eq!(
        validate_exclusion_recovery_receipt(&exclusion, &substituted).unwrap_err(),
        NativeFamilyReceiptError::PopulationMismatch {
            population: PopulationId::ThreeDocumentSlice.as_str().to_owned()
        }
    );

    // A truncated denominator: one exception row dropped.
    let mut truncated = exclusion.clone();
    truncated.recoveries.pop();
    assert_eq!(
        validate_exclusion_recovery_receipt(&truncated, &exclusion_expectation).unwrap_err(),
        NativeFamilyReceiptError::ExclusionPopulationCoverage {
            expected: 79,
            found: 78
        }
    );

    // A denominator whose reason set is incomplete.
    let mut incomplete = exclusion_expectation.clone();
    incomplete.population.reasons.remove("header_author_absent");
    assert_eq!(
        validate_exclusion_recovery_receipt(&exclusion, &incomplete).unwrap_err(),
        NativeFamilyReceiptError::ExclusionDenominatorAbsent {
            reason: "header_author_absent".to_owned()
        }
    );

    // A stale declaration pin (the receipt is bound to older bytes).
    let mut stale = parity_expectation.clone();
    stale.corpus.revision_sha256 = sha256_prefixed(b"stale declaration");
    assert_eq!(
        validate_parity_receipt(&parity, &stale).unwrap_err(),
        NativeFamilyReceiptError::BindingDrift {
            field: "declaration.revision_sha256".to_owned(),
            expected: sha256_prefixed(b"stale declaration"),
            found: sha256_prefixed(LAYOUT.as_bytes()),
        }
    );

    // A stale oracle pin.
    let mut stale_oracle = parity_expectation.clone();
    stale_oracle.oracle_revision_sha256 = sha256_prefixed(b"stale provenance");
    assert_eq!(
        validate_parity_receipt(&parity, &stale_oracle).unwrap_err(),
        NativeFamilyReceiptError::BindingDrift {
            field: "oracle.revision_sha256".to_owned(),
            expected: sha256_prefixed(b"stale provenance"),
            found: sha256_prefixed(PROVENANCE.as_bytes()),
        }
    );

    // A threshold claim.
    let mut threshold = parity.clone();
    threshold.thresholds = "accepted".to_owned();
    assert_eq!(
        validate_parity_receipt(&threshold, &parity_expectation).unwrap_err(),
        NativeFamilyReceiptError::ThresholdClaimed {
            found: "accepted".to_owned()
        }
    );

    // A product-coverage claim.
    let mut product = parity.clone();
    product.product_coverage_effect = "full".to_owned();
    assert_eq!(
        validate_parity_receipt(&product, &parity_expectation).unwrap_err(),
        NativeFamilyReceiptError::ProductCoverageClaimed {
            found: "full".to_owned()
        }
    );

    // A widened claim scope.
    let mut widened = parity.clone();
    widened.claim_scope = "product_ready".to_owned();
    assert_eq!(
        validate_parity_receipt(&widened, &parity_expectation).unwrap_err(),
        NativeFamilyReceiptError::ClaimScopeDrift {
            found: "product_ready".to_owned()
        }
    );

    // A pooled bibliography (R128).
    let mut pooled = parity.clone();
    pooled.pooling.citation_names_pooled_with_biblstruct = true;
    assert_eq!(
        validate_parity_receipt(&pooled, &parity_expectation).unwrap_err(),
        NativeFamilyReceiptError::PoolingClaimed {
            field: "citation_names_pooled_with_biblstruct".to_owned()
        }
    );

    // A raised redaction flag.
    let mut leaked = parity.clone();
    leaked.redaction.raw_text = true;
    assert_eq!(
        validate_parity_receipt(&leaked, &parity_expectation).unwrap_err(),
        NativeFamilyReceiptError::RedactionViolation {
            marker: "redaction flag set".to_owned()
        }
    );

    // A corpus count that drifted from the documents it restates.
    let mut count = parity_expectation.clone();
    count.corpus.document_count = 13;
    assert_eq!(
        validate_parity_receipt(&parity, &count).unwrap_err(),
        NativeFamilyReceiptError::CorpusCountDrift {
            expected: 12,
            found: 13
        }
    );

    // A corpus page denominator that drifted from the pinned documents.
    let mut pages = parity_expectation.clone();
    pages.corpus.page_denominator = 382;
    assert_eq!(
        validate_parity_receipt(&parity, &pages).unwrap_err(),
        NativeFamilyReceiptError::CorpusPageDrift {
            expected: 381,
            found: 382
        }
    );

    // The committed artifacts are untouched: every mutation was an in-memory
    // clone, and the tracked bytes still canonicalize.
    assert_eq!(
        std::str::from_utf8(
            &parity_receipt()
                .artifact_json_bytes()
                .expect("the committed parity receipt still serializes")
        )
        .expect("artifact is UTF-8"),
        PARITY_ARTIFACT
    );
    assert_eq!(
        std::str::from_utf8(
            &exclusion_receipt()
                .artifact_json_bytes()
                .expect("the committed exclusion receipt still serializes")
        )
        .expect("artifact is UTF-8"),
        EXCLUSION_ARTIFACT
    );
}

// ─────────────────────────── hermeticity ───────────────────────────

#[test]
fn the_audit_reads_no_gitignored_corpus() {
    // Every corpus path the receipts carry is a pin, not an input: the audit
    // resolves none of them, and all of its own inputs are git-tracked DATA.
    // This is the S03/T05 lesson — a gate that reads `data/sources/**/*.pdf`
    // does not run from a clean checkout.
    let tracked_inputs = [
        "artifacts/parse_compat/native_family_parity.json",
        "artifacts/parse_compat/native_exclusion_recovery.json",
        "data/parse/layout_bakeoff.yaml",
        "data/parse/grobid_compatibility.yaml",
        "data/parse/holdout_manifests.yaml",
        "artifacts/parse_compat/external_holdout_release_probe.json",
        "artifacts/parse_compat/absorption_progress_baseline.json",
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
    // they are evidence of what the producers read, and the audit never opens them.
    let parity = parity_receipt();
    let exclusion = exclusion_receipt();
    for document in &population_of(&parity).documents {
        assert!(document.source_pdf.starts_with("data/sources/"));
        assert!(Path::new(&document.source_pdf).is_relative());
    }
    for row in &exclusion.recoveries {
        if let Some(source_pdf) = &row.source_pdf {
            assert!(source_pdf.starts_with("data/sources/"));
            assert!(Path::new(source_pdf).is_relative());
        }
    }

    // The authority locks are carried, not relaxed.
    assert_eq!(
        parity.locks.get("deployed").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        parity.locks.get("import_eligible").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        parity.locks.get("auto_promote").map(String::as_str),
        Some("forbidden")
    );
    assert_eq!(
        parity.locks.get("graph_write").map(String::as_str),
        Some("forbidden")
    );
    assert_eq!(exclusion.locks, parity.locks);
}
