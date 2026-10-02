//! S08/T03 (M514-1lss2s) — corpus-bound same-n resource receipt.
//!
//! The S06/T06 receipt ([`crate::native_resource_same_n`]) measured resource
//! costs on the three pinned documents of the declared Java-free slice. S08 is
//! the independent acceptance over the **frozen 610-document holdout**, and the
//! roadmap line requires a resource comparison on *that* corpus. This module
//! owns the honest corpus-bound deliverable: it binds the native resources the
//! T01 acceptance runner actually measured over the 610 assigned documents
//! ([`crate::native_acceptance`]) and pairs each metric with a reference row
//! **only where a pinned same-n source covers exactly the same document set**.
//! No such reference exists for the 610-corpus, so every reference metric is
//! emitted `not_observed` with a closed reason (D510) — never a restated Java
//! number, never a pooled value, never a saving claim.
//!
//! ## The pairing rule is the contract
//!
//! The S06 pairing frame is reused verbatim: `rule = same_documents_only`,
//! `pooled = forbidden`, `saving_claim = forbidden`,
//! `absence_policy = typed_not_observed`, `mismatch_disposition = hard_failure`.
//! A document set that disagrees with the frozen holdout manifest is refused by
//! name ([`SameNReceiptError::Mismatch`]).
//!
//! ## A zero is not a measurement
//!
//! The native rows are the T01 population metrics (numerator/denominator); an
//! `observed` native row with an empty denominator is refused
//! ([`SameNReceiptError::ZeroMasquerade`]). The reference rows are either
//! `observed` (a value and a pinned same-n source) or `not_observed` (a closed
//! reason and no value); an untyped reason is refused
//! ([`SameNReceiptError::AbsenceReasonUnknown`]) and an observed reference row
//! whose carrier is not pinned same-n is refused
//! ([`SameNReceiptError::ReferenceSourceUnpinned`]).
//!
//! ## Envelope reuse
//!
//! The claim envelope (`native_family_receipt`), the prospective limit freeze
//! binding (`native_route_receipt`) and the S06 pairing/absence vocabulary and
//! observation types (`native_resource_same_n`) are reused rather than
//! restated. The module is pure `std`/`serde`/`serde_yaml`/`serde_json`/`sha2`;
//! it and its contract test compile in the default feature set. It reads
//! committed carriers only: no `pdf-layout`, no `pdf-classifier`, no `da-parse`,
//! no Java, no network.

use crate::native_acceptance::{
    AcceptanceDocumentRow, AcceptanceGuard, AcceptanceMetricRow, AcceptancePopulations,
    NativeAcceptanceReceipt,
};
use crate::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, CarrierBinding, LEAK_MARKERS, PRODUCT_COVERAGE_EFFECT_NONE,
    REQUIRED_LOCKS, Redaction, THRESHOLDS_NOT_ACCEPTED, is_repo_relative_path, is_sha256_digest,
    is_sha256_hex, sha256_hex, sha256_prefixed,
};
use crate::native_resource_same_n::{
    SAME_N_ABSENCE_POLICY, SAME_N_ABSENCE_REASON_VOCABULARY, SAME_N_MISMATCH_HARD_FAILURE,
    SAME_N_OBSERVATION_STATE_VOCABULARY, SAME_N_PAIRING_RULE, SAME_N_POOLED_FORBIDDEN,
    SAME_N_REFERENCE_METRIC_ORDER, SAME_N_SAVING_CLAIM_FORBIDDEN, SameNMetricObservation,
    SameNPairing, SameNReceiptError, SameNResiduals,
};
use crate::native_route_receipt::{
    ROUTE_DIRECTION_VOCABULARY, ROUTE_FREEZE_SCOPE, ROUTE_FREEZE_STATE_FROZEN,
    ROUTE_SILENT_CLAMP_FORBIDDEN, ROUTE_TUNED_AFTER_SCORING_FORBIDDEN, RouteFreezeDeclaration,
    RouteLimitsBinding,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Schema version of the emitted corpus-bound same-n resource artifact.
pub const NATIVE_CORPUS_RESOURCE_SAME_N_SCHEMA_VERSION: &str = "native-corpus-resource-same-n.v1";
/// Default artifact path of the corpus-bound same-n resource receipt.
pub const DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL: &str =
    "artifacts/parse_compat/native_corpus_resource_same_n.json";
/// Repository-relative carrier of the T01 corpus-bound native measurement.
pub const CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL: &str =
    "artifacts/parse_compat/native_corpus_acceptance.json";
/// Repository-relative carrier of the pinned reference work rows (S06 corpus).
pub const CORPUS_RESOURCE_REFERENCE_WORK_REL: &str = "artifacts/parse_compat/native_java_work.json";
/// Repository-relative carrier of the Java batch-budget live observation.
pub const CORPUS_RESOURCE_REFERENCE_BUDGET_REL: &str =
    "artifacts/parse_compat/parse_batch_budget_live.json";
/// Repository-relative carrier of the committed route receipt holding the freeze.
pub const CORPUS_RESOURCE_ROUTE_RECEIPT_REL: &str = "artifacts/parse_compat/native_route_e2e.json";
/// Repository-relative carrier of the governor freeze block.
pub const CORPUS_RESOURCE_GOVERNOR_REL: &str =
    "scripts/governor/checks/grobid_absorption_progress.yaml";
/// Pointer inside the governor carrier at the route freeze block.
pub const CORPUS_RESOURCE_GOVERNOR_POINTER: &str = "native_route_e2e";
/// Repository-relative carrier of the frozen holdout manifest.
pub const CORPUS_RESOURCE_HOLDOUT_MANIFEST_REL: &str = "data/parse/holdout_manifests.yaml";
/// Pointer inside the holdout manifest at the assigned article census.
pub const CORPUS_RESOURCE_HOLDOUT_POINTER: &str = "articles";
/// The closed reason for a carrier that is not same-n to the 610 assigned set.
pub const CORPUS_RESOURCE_REFERENCE_NOT_SAME_N: &str = "reference_row_not_same_n";
/// The declared measurement scope of the native population.
pub const CORPUS_RESOURCE_NATIVE_SCOPE: &str = "native_plane_extraction";
/// The declared measurement scope of the reference population.
pub const CORPUS_RESOURCE_REFERENCE_SCOPE: &str = "java_crf_fulltext";

// ───────────────────────────── types ─────────────────────────────

/// The pinned identity of one assigned document, re-derived from the frozen
/// holdout manifest (no page count: the manifest carries none for excluded
/// documents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignedIdentity {
    /// Pinned document id.
    pub id: String,
    /// Repository-relative source PDF.
    pub source_pdf: String,
    /// Prefixed sha256 of the pinned PDF bytes.
    pub sha256: String,
}

/// A pinned reference carrier examined for pairing, and whether it is same-n to
/// the 610 assigned corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusResourceReferenceCarrier {
    /// Binding to the examined carrier.
    pub carrier: CarrierBinding,
    /// `true` only when the carrier covers exactly the 610 assigned set.
    pub same_n: bool,
    /// The closed reason the carrier is not same-n; empty when `same_n`.
    pub reason: String,
}

/// The emitted corpus-bound same-n resource receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusResourceSameNReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// `diagnostic_only`.
    pub claim_scope: String,
    /// `none`.
    pub product_coverage_effect: String,
    /// `not_accepted`.
    pub thresholds: String,
    /// Locks as declared, after the required-lock check.
    pub locks: BTreeMap<String, String>,
    /// Guard provenance inherited from the T01 measurement.
    pub guard: AcceptanceGuard,
    /// The prospective limit freeze binding (reused from the route receipt).
    pub limits: RouteLimitsBinding,
    /// Binding to the governor freeze block the revision was recomputed from.
    pub freeze_governor: CarrierBinding,
    /// The declared pairing frame (reused from the S06 receipt).
    pub pairing: SameNPairing,
    /// Binding to the frozen holdout manifest that names the assigned set.
    pub assigned_source: CarrierBinding,
    /// Binding to the T01 carrier that measured the native resources.
    pub native_measurement: CarrierBinding,
    /// The frozen denominator partition, carried from the T01 measurement.
    pub populations: AcceptancePopulations,
    /// The native resources T01 actually measured, with non-empty denominators.
    pub native_metrics: Vec<AcceptanceMetricRow>,
    /// The reference carriers examined, each named as same-n or not.
    pub reference_carriers: Vec<CorpusResourceReferenceCarrier>,
    /// The reference metric rows; absent here because no same-n source exists.
    pub reference_metrics: Vec<SameNMetricObservation>,
    /// One typed row per assigned document, in carrier order.
    pub documents: Vec<AcceptanceDocumentRow>,
    /// The closure counters the contract test recomputes.
    pub residuals: SameNResiduals,
    /// Redaction invariants.
    pub redaction: Redaction,
}

impl CorpusResourceSameNReceipt {
    /// Parse a receipt from canonical bytes, refusing unknown fields.
    pub fn parse_json_bytes(bytes: &[u8]) -> Result<Self, SameNReceiptError> {
        serde_json::from_slice(bytes).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })
    }

    /// The canonical artifact bytes, after redaction is asserted.
    pub fn artifact_json_bytes(&self) -> Result<Vec<u8>, SameNReceiptError> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|error| SameNReceiptError::Malformed {
                detail: error.to_string(),
            })?;
        let text = std::str::from_utf8(&bytes).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
        assert_corpus_resource_redacted(text)?;
        Ok(bytes)
    }
}

/// Refuse any leaked marker in the serialized receipt.
pub fn assert_corpus_resource_redacted(json: &str) -> Result<(), SameNReceiptError> {
    for marker in LEAK_MARKERS {
        if json.contains(marker) {
            return Err(SameNReceiptError::RedactionLeak {
                marker: marker.to_owned(),
            });
        }
    }
    Ok(())
}

/// The builder input: the parsed T01 measurement plus the committed bindings.
#[derive(Debug, Clone)]
pub struct CorpusResourceSameNInput {
    /// Binding to the frozen holdout manifest.
    pub assigned_source: CarrierBinding,
    /// Binding to the T01 carrier.
    pub native_measurement: CarrierBinding,
    /// The parsed T01 acceptance receipt.
    pub native_receipt: NativeAcceptanceReceipt,
    /// The assigned identities re-derived from the holdout manifest.
    pub assigned: Vec<AssignedIdentity>,
    /// Binding to the governor freeze block.
    pub freeze_governor: CarrierBinding,
    /// The freeze declaration the limits must match.
    pub freeze: RouteFreezeDeclaration,
    /// The frozen limits binding.
    pub limits: RouteLimitsBinding,
    /// The examined reference carriers.
    pub reference_carriers: Vec<CorpusResourceReferenceCarrier>,
    /// The reference metric rows.
    pub reference_metrics: Vec<SameNMetricObservation>,
    /// The declared claim scope.
    pub claim_scope: String,
    /// The declared product-coverage effect.
    pub product_coverage_effect: String,
    /// The declared thresholds state.
    pub thresholds: String,
}

/// What the validator recomputes and compares against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusResourceSameNExpectation {
    /// The composite freeze revision recomputed from the committed carriers.
    pub freeze_revision: String,
    /// The T01 carrier binding the receipt must name.
    pub native_measurement: CarrierBinding,
    /// The assigned identities the document rows must match exactly.
    pub assigned: Vec<AssignedIdentity>,
}

// ───────────────────────────── builder ─────────────────────────────

/// Build the receipt, refusing every contract violation by name.
pub fn build_corpus_resource_same_n_receipt(
    input: &CorpusResourceSameNInput,
) -> Result<CorpusResourceSameNReceipt, SameNReceiptError> {
    let locks = required_locks();
    check_locks(&locks)?;
    check_claim(
        &input.claim_scope,
        &input.product_coverage_effect,
        &input.thresholds,
    )?;
    check_limits(&input.limits)?;
    if input.freeze.freeze_revision != input.limits.freeze_revision
        || input.freeze.metrics != input.limits.metrics
    {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!(
                "limits {} do not match the declared freeze {}",
                input.limits.freeze_revision, input.freeze.freeze_revision
            ),
        });
    }
    check_guard(&input.native_receipt.guard)?;

    // The native document rows must be exactly the assigned holdout census.
    check_document_set(&input.assigned, &input.native_receipt.documents)?;
    if input.native_receipt.populations.assigned != input.assigned.len() as u64 {
        return Err(SameNReceiptError::CorpusDrift {
            detail: format!(
                "T01 partition assigned {} but the manifest assigns {}",
                input.native_receipt.populations.assigned,
                input.assigned.len()
            ),
        });
    }

    // Every native row is an observation; an empty denominator is a silent zero.
    check_no_native_duplicates(&input.native_receipt.metrics)?;
    for row in &input.native_receipt.metrics {
        if row.metric_id.trim().is_empty() {
            return Err(SameNReceiptError::MetricUnknown {
                metric: "<empty>".to_owned(),
            });
        }
        if row.denominator == 0 {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.metric_id.clone(),
                detail: "an observed native row carries an empty denominator".to_owned(),
            });
        }
    }

    // Every reference carrier must be pinned and typed same-n or not.
    check_reference_carriers(&input.reference_carriers)?;
    // Every reference row must be closed and, when observed, bound to a pinned
    // same-n carrier.
    check_reference_metrics(&input.reference_metrics, &input.reference_carriers)?;

    let residuals = residuals_of(&input.native_receipt.metrics, &input.reference_metrics);

    Ok(CorpusResourceSameNReceipt {
        schema_version: NATIVE_CORPUS_RESOURCE_SAME_N_SCHEMA_VERSION.to_owned(),
        claim_scope: input.claim_scope.clone(),
        product_coverage_effect: input.product_coverage_effect.clone(),
        thresholds: input.thresholds.clone(),
        locks,
        guard: input.native_receipt.guard.clone(),
        limits: input.limits.clone(),
        freeze_governor: input.freeze_governor.clone(),
        pairing: SameNPairing {
            rule: SAME_N_PAIRING_RULE.to_owned(),
            pooled: SAME_N_POOLED_FORBIDDEN.to_owned(),
            saving_claim: SAME_N_SAVING_CLAIM_FORBIDDEN.to_owned(),
            absence_policy: SAME_N_ABSENCE_POLICY.to_owned(),
            mismatch_disposition: SAME_N_MISMATCH_HARD_FAILURE.to_owned(),
        },
        assigned_source: input.assigned_source.clone(),
        native_measurement: input.native_measurement.clone(),
        populations: input.native_receipt.populations.clone(),
        native_metrics: input.native_receipt.metrics.clone(),
        reference_carriers: input.reference_carriers.clone(),
        reference_metrics: input.reference_metrics.clone(),
        documents: input.native_receipt.documents.clone(),
        residuals,
        redaction: Redaction::clean(),
    })
}

// ───────────────────────────── validator ─────────────────────────────

/// Independently re-read a parsed receipt and refuse what a builder cannot
/// express: a stale freeze revision, a reopened lock, a document-set drift, an
/// empty native denominator, or a pooled/saving claim.
pub fn validate_corpus_resource_same_n_receipt(
    receipt: &CorpusResourceSameNReceipt,
    expectation: &CorpusResourceSameNExpectation,
) -> Result<(), SameNReceiptError> {
    if receipt.schema_version != NATIVE_CORPUS_RESOURCE_SAME_N_SCHEMA_VERSION {
        return Err(SameNReceiptError::SchemaDrift {
            field: "schema_version".to_owned(),
            expected: NATIVE_CORPUS_RESOURCE_SAME_N_SCHEMA_VERSION.to_owned(),
            found: receipt.schema_version.clone(),
        });
    }
    if receipt.limits.freeze_revision != expectation.freeze_revision {
        return Err(SameNReceiptError::StaleFreezeRevision {
            expected: expectation.freeze_revision.clone(),
            found: receipt.limits.freeze_revision.clone(),
        });
    }
    check_claim(
        &receipt.claim_scope,
        &receipt.product_coverage_effect,
        &receipt.thresholds,
    )?;
    check_locks(&receipt.locks)?;
    check_limits(&receipt.limits)?;

    if receipt.pairing.rule != SAME_N_PAIRING_RULE {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.rule.clone(),
        });
    }
    if receipt.pairing.pooled != SAME_N_POOLED_FORBIDDEN {
        return Err(SameNReceiptError::PooledClaimed {
            found: receipt.pairing.pooled.clone(),
        });
    }
    if receipt.pairing.saving_claim != SAME_N_SAVING_CLAIM_FORBIDDEN {
        return Err(SameNReceiptError::SavingClaimed {
            found: receipt.pairing.saving_claim.clone(),
        });
    }
    if receipt.pairing.absence_policy != SAME_N_ABSENCE_POLICY {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.absence_policy.clone(),
        });
    }
    if receipt.pairing.mismatch_disposition != SAME_N_MISMATCH_HARD_FAILURE {
        return Err(SameNReceiptError::PairingRuleDrift {
            found: receipt.pairing.mismatch_disposition.clone(),
        });
    }

    check_guard(&receipt.guard)?;

    if receipt.native_measurement != expectation.native_measurement {
        return Err(SameNReceiptError::Mismatch {
            detail: "the receipt names a different T01 measurement carrier".to_owned(),
        });
    }

    // The document set is the frozen assigned census: `mismatch = hard_failure`.
    check_document_set(&expectation.assigned, &receipt.documents)?;
    if receipt.populations.assigned != expectation.assigned.len() as u64 {
        return Err(SameNReceiptError::CorpusDrift {
            detail: format!(
                "populations.assigned {} but the manifest assigns {}",
                receipt.populations.assigned,
                expectation.assigned.len()
            ),
        });
    }

    check_no_native_duplicates(&receipt.native_metrics)?;
    for row in &receipt.native_metrics {
        if row.denominator == 0 {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.metric_id.clone(),
                detail: "an observed native row carries an empty denominator".to_owned(),
            });
        }
    }

    check_reference_carriers(&receipt.reference_carriers)?;
    check_reference_metrics(&receipt.reference_metrics, &receipt.reference_carriers)?;

    assert_redaction(&receipt.redaction)?;
    let re_serialized =
        serde_json::to_vec(receipt).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
    let re_serialized =
        std::str::from_utf8(&re_serialized).map_err(|error| SameNReceiptError::Malformed {
            detail: error.to_string(),
        })?;
    assert_corpus_resource_redacted(re_serialized)?;

    let recomputed = residuals_of(&receipt.native_metrics, &receipt.reference_metrics);
    if recomputed != receipt.residuals {
        return Err(SameNReceiptError::ZeroMasquerade {
            metric: "residuals".to_owned(),
            detail: format!(
                "declared {:?}, recomputed {:?}",
                receipt.residuals, recomputed
            ),
        });
    }

    Ok(())
}

// ───────────────────────────── helpers ─────────────────────────────

/// Recompute the composite freeze revision the governor block declares:
/// `sha256(join("<path>:<sha256-hex>", "\n"))` over the ordered carriers.
pub fn freeze_revision(carriers: &[(String, String)]) -> String {
    let lines: Vec<String> = carriers
        .iter()
        .map(|(path, hex)| format!("{path}:{hex}"))
        .collect();
    sha256_prefixed(lines.join("\n").as_bytes())
}

/// The bare hex digest of a committed carrier, as the freeze line format needs.
pub fn carrier_bare_digest(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

fn required_locks() -> BTreeMap<String, String> {
    REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect()
}

fn check_locks(locks: &BTreeMap<String, String>) -> Result<(), SameNReceiptError> {
    for (lock, expected) in REQUIRED_LOCKS {
        match locks.get(lock) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(SameNReceiptError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(SameNReceiptError::LockOpened {
                    lock: (*lock).to_owned(),
                    found: "<absent>".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn check_claim(
    claim_scope: &str,
    product_coverage_effect: &str,
    thresholds: &str,
) -> Result<(), SameNReceiptError> {
    if claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(SameNReceiptError::ClaimScopeDrift {
            found: claim_scope.to_owned(),
        });
    }
    if product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(SameNReceiptError::ProductCoverageClaimed {
            found: product_coverage_effect.to_owned(),
        });
    }
    if thresholds != THRESHOLDS_NOT_ACCEPTED {
        return Err(SameNReceiptError::ThresholdClaimed {
            found: thresholds.to_owned(),
        });
    }
    Ok(())
}

fn check_limits(limits: &RouteLimitsBinding) -> Result<(), SameNReceiptError> {
    if limits.freeze_state != ROUTE_FREEZE_STATE_FROZEN {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_state `{}`", limits.freeze_state),
        });
    }
    if limits.freeze_scope != ROUTE_FREEZE_SCOPE {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_scope `{}`", limits.freeze_scope),
        });
    }
    if limits.tuned_after_scoring != ROUTE_TUNED_AFTER_SCORING_FORBIDDEN {
        return Err(SameNReceiptError::LimitTunedAfterScoring {
            found: limits.tuned_after_scoring.clone(),
        });
    }
    if limits.silent_clamp != ROUTE_SILENT_CLAMP_FORBIDDEN {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("silent_clamp `{}`", limits.silent_clamp),
        });
    }
    if limits.claim_without_freeze != "forbidden" {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("claim_without_freeze `{}`", limits.claim_without_freeze),
        });
    }
    if !limits.zero_is_not_a_measurement || !limits.pass_requires_frozen_threshold {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: "zero_is_not_a_measurement / pass_requires_frozen_threshold must be true"
                .to_owned(),
        });
    }
    if limits.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: "direction vocabulary drifted".to_owned(),
        });
    }
    if !is_sha256_digest(&limits.freeze_revision) {
        return Err(SameNReceiptError::LimitNotFrozen {
            detail: format!("freeze_revision `{}`", limits.freeze_revision),
        });
    }
    Ok(())
}

fn check_guard(guard: &AcceptanceGuard) -> Result<(), SameNReceiptError> {
    if guard.observed_java_calls != 0 {
        return Err(SameNReceiptError::ObservedJavaOnRoute {
            found: guard.observed_java_calls,
        });
    }
    if !guard.installed || !guard.identity_isolated || !guard.positive_control {
        return Err(SameNReceiptError::GuardMissingForObservedZero {
            detail: format!(
                "installed={}, identity_isolated={}, positive_control={}",
                guard.installed, guard.identity_isolated, guard.positive_control
            ),
        });
    }
    Ok(())
}

/// The document rows must be exactly the assigned holdout census, by id, source
/// and sha256. A difference is a hard failure, named by document.
fn check_document_set(
    assigned: &[AssignedIdentity],
    documents: &[AcceptanceDocumentRow],
) -> Result<(), SameNReceiptError> {
    if assigned.len() != documents.len() {
        return Err(SameNReceiptError::Mismatch {
            detail: format!(
                "{} assigned identities vs {} document rows",
                assigned.len(),
                documents.len()
            ),
        });
    }
    let by_id: BTreeMap<&str, &AssignedIdentity> = assigned
        .iter()
        .map(|identity| (identity.id.as_str(), identity))
        .collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for document in documents {
        let identity =
            by_id
                .get(document.id.as_str())
                .ok_or_else(|| SameNReceiptError::Mismatch {
                    detail: format!("document `{}` is not in the assigned census", document.id),
                })?;
        if !seen.insert(document.id.as_str()) {
            return Err(SameNReceiptError::Mismatch {
                detail: format!("document `{}` is declared more than once", document.id),
            });
        }
        if identity.sha256 != document.sha256 {
            return Err(SameNReceiptError::Mismatch {
                detail: format!("document `{}` sha256 drifted", document.id),
            });
        }
        if identity.source_pdf != document.source_pdf {
            return Err(SameNReceiptError::Mismatch {
                detail: format!("document `{}` source_pdf drifted", document.id),
            });
        }
    }
    for identity in assigned {
        if !seen.contains(identity.id.as_str()) {
            return Err(SameNReceiptError::Mismatch {
                detail: format!("assigned `{}` has no document row", identity.id),
            });
        }
    }
    Ok(())
}

fn check_no_native_duplicates(rows: &[AcceptanceMetricRow]) -> Result<(), SameNReceiptError> {
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(row.metric_id.as_str()) {
            return Err(SameNReceiptError::MetricDuplicate {
                metric: row.metric_id.clone(),
            });
        }
    }
    Ok(())
}

/// A reference carrier must be pinned and typed: a non-same-n carrier carries a
/// closed reason, a same-n carrier carries none.
fn check_reference_carriers(
    carriers: &[CorpusResourceReferenceCarrier],
) -> Result<(), SameNReceiptError> {
    for entry in carriers {
        let binding = &entry.carrier;
        if !is_repo_relative_path(&binding.manifest) {
            return Err(SameNReceiptError::ReferenceSourceUnpinned {
                metric: "<carrier>".to_owned(),
                detail: format!("manifest `{}` is not repository-relative", binding.manifest),
            });
        }
        if !is_sha256_digest(&binding.revision_sha256) {
            return Err(SameNReceiptError::ReferenceSourceUnpinned {
                metric: "<carrier>".to_owned(),
                detail: format!("sha256 `{}` is not a digest", binding.revision_sha256),
            });
        }
        if binding.pointer.is_empty() {
            return Err(SameNReceiptError::ReferenceSourceUnpinned {
                metric: "<carrier>".to_owned(),
                detail: "carrier carries no pointer".to_owned(),
            });
        }
        if entry.same_n {
            if !entry.reason.is_empty() {
                return Err(SameNReceiptError::ReferenceSourceUnpinned {
                    metric: "<carrier>".to_owned(),
                    detail: "a same-n carrier must not carry an absence reason".to_owned(),
                });
            }
        } else if !SAME_N_ABSENCE_REASON_VOCABULARY.contains(&entry.reason.as_str()) {
            return Err(SameNReceiptError::AbsenceReasonUnknown {
                metric: "<carrier>".to_owned(),
                reason: entry.reason.clone(),
            });
        }
    }
    Ok(())
}

fn check_reference_metrics(
    rows: &[SameNMetricObservation],
    carriers: &[CorpusResourceReferenceCarrier],
) -> Result<(), SameNReceiptError> {
    let mut seen = BTreeSet::new();
    for row in rows {
        if !SAME_N_REFERENCE_METRIC_ORDER.contains(&row.id.as_str()) {
            return Err(SameNReceiptError::MetricUnknown {
                metric: row.id.clone(),
            });
        }
        if !seen.insert(row.id.as_str()) {
            return Err(SameNReceiptError::MetricDuplicate {
                metric: row.id.clone(),
            });
        }
        check_observation_shape(row)?;
        if row.is_observed() {
            check_reference_source(row, carriers)?;
        }
    }
    Ok(())
}

/// `not_observed` carries no value and a closed reason; `observed` carries a
/// value and a source.
fn check_observation_shape(row: &SameNMetricObservation) -> Result<(), SameNReceiptError> {
    if !SAME_N_OBSERVATION_STATE_VOCABULARY.contains(&row.state.as_str()) {
        return Err(SameNReceiptError::ZeroMasquerade {
            metric: row.id.clone(),
            detail: format!("unknown observation state `{}`", row.state),
        });
    }
    if row.state == "observed" {
        if row.value.is_none() {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.id.clone(),
                detail: "an observed row carries no value".to_owned(),
            });
        }
        if row.reason.is_some() {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.id.clone(),
                detail: "an observed row carries an absence reason".to_owned(),
            });
        }
        if row.source.is_none() {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.id.clone(),
                detail: "an observed row carries no source".to_owned(),
            });
        }
    } else {
        if row.value.is_some() {
            return Err(SameNReceiptError::ZeroMasquerade {
                metric: row.id.clone(),
                detail: "a not_observed row carries a number".to_owned(),
            });
        }
        let reason =
            row.reason
                .as_deref()
                .ok_or_else(|| SameNReceiptError::AbsenceReasonUnknown {
                    metric: row.id.clone(),
                    reason: "<absent>".to_owned(),
                })?;
        if !SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason) {
            return Err(SameNReceiptError::AbsenceReasonUnknown {
                metric: row.id.clone(),
                reason: reason.to_owned(),
            });
        }
    }
    Ok(())
}

/// An observed reference row must name a pinned measured carrier that the
/// receipt declares same-n. No same-n carrier exists for the 610-corpus, so any
/// observed row is refused here.
fn check_reference_source(
    row: &SameNMetricObservation,
    carriers: &[CorpusResourceReferenceCarrier],
) -> Result<(), SameNReceiptError> {
    let source = row
        .source
        .as_ref()
        .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: "no source".to_owned(),
        })?;
    if source.kind != "measured_reference" {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("source kind `{}`", source.kind),
        });
    }
    let manifest =
        source
            .manifest
            .as_deref()
            .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
                metric: row.id.clone(),
                detail: "no manifest".to_owned(),
            })?;
    if !is_repo_relative_path(manifest) {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("manifest `{manifest}` is not repository-relative"),
        });
    }
    let sha256 =
        source
            .sha256
            .as_deref()
            .ok_or_else(|| SameNReceiptError::ReferenceSourceUnpinned {
                metric: row.id.clone(),
                detail: "no sha256".to_owned(),
            })?;
    if !is_sha256_hex(sha256) {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("sha256 `{sha256}` is not 64 hex chars"),
        });
    }
    if source.pointer.as_deref().unwrap_or_default().is_empty() {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: "no pointer".to_owned(),
        });
    }
    if source.measurement_state.as_deref() != Some("measured") {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!(
                "measurement_state `{}`",
                source.measurement_state.as_deref().unwrap_or("<absent>")
            ),
        });
    }
    let same_n = carriers
        .iter()
        .any(|entry| entry.same_n && entry.carrier.manifest == manifest);
    if !same_n {
        return Err(SameNReceiptError::ReferenceSourceUnpinned {
            metric: row.id.clone(),
            detail: format!("carrier `{manifest}` is not declared same-n"),
        });
    }
    Ok(())
}

fn residuals_of(
    native_metrics: &[AcceptanceMetricRow],
    reference_metrics: &[SameNMetricObservation],
) -> SameNResiduals {
    // Every native metric row is a measurement with a non-empty denominator.
    let mut observed = native_metrics.len() as u64;
    let mut absent = 0u64;
    let mut untyped = 0u64;
    let mut bound = 0u64;
    for row in reference_metrics {
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
    SameNResiduals {
        observed_metrics: observed,
        absent_metrics: absent,
        untyped_metrics: untyped,
        reference_rows_bound: bound,
    }
}

fn assert_redaction(redaction: &Redaction) -> Result<(), SameNReceiptError> {
    let flags = [
        ("raw_text", redaction.raw_text),
        ("raw_pdf", redaction.raw_pdf),
        ("tei_xml", redaction.tei_xml),
        ("images", redaction.images),
        ("physical_paths", redaction.physical_paths),
        ("hostname", redaction.hostname),
        ("executable", redaction.executable),
    ];
    for (field, value) in flags {
        if value {
            return Err(SameNReceiptError::RedactionFlag {
                field: field.to_owned(),
                found: value,
            });
        }
    }
    Ok(())
}
