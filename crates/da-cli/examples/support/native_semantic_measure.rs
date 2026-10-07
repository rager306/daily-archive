//! M516-th0oby / S04 / T01 — measurement-plane helper for the per-document
//! semantic-correctness receipt.
//!
//! This module folds the expanded route corpus's 12 committed reference TEI
//! through the *composer* — the same `compose_native_fulltext` subject the
//! frozen three-document H14/H15 oracle measures — and records, per document, a
//! closed well-formedness reason and a closed bibliography-target reason. It then
//! reads the committed native route emission envelope hermetically (no network,
//! no PDF) to disclose the native-route-emission scope beside the composer facts.
//!
//! Like `native_compose.rs`, it deliberately lives outside `crates/da-cli/src`:
//! it names the quarantined composer's identifiers, and the one call site that
//! must compose a document to *measure* it stays in the example/test plane
//! (D381). It is `#[path]`-shared by the example runner and the contract test, so
//! both measure the identical path.
//!
//! Nothing is hand-typed: every reference TEI is sha256-verified against the
//! pinned DATA block before it is measured, and the emission envelope is bound by
//! its own digest. A missing or drifted fixture refuses by name rather than
//! shrinking the denominator.

#[path = "native_compose.rs"]
mod native_compose;

use self::native_compose::{NativeCompositionMeasurement, compose_native_document_typed};
use da_cli::native_composition_measurement::{build_admitted_band, measure_source};
use da_cli::native_family_receipt::sha256_prefixed;
use da_cli::native_route_semantic_receipt::{
    AB_ACCEPTED, BAND_BUILD_ADMITTED_BAND_FAILED, BAND_MEASURE_SOURCE_FAILED, COMPOSER_ACCEPTED,
    COMPOSER_REFUSED, DEFAULT_EMISSION_ARTIFACT_REL, MeasuredSemanticDocument,
    NativeRouteSemanticReceipt, ReferenceLedgerView, admitted_band_outcome_token,
    build_semantic_receipt, derive_semantic_receipt_expectation, profile_class_for_path,
};
use serde_yaml::Value as Yaml;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Repository-relative carriers the measurement reads.
const LAYOUT_REL: &str = "data/parse/layout_bakeoff.yaml";
const ACCEPTANCE_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
/// The closed composer-refusal class recorded when the admitted band itself
/// could not be produced (the composer was never called).
const BAND_UNAVAILABLE_CLASS: &str = "admitted_band_unavailable";

/// The built receipt plus the canonical bytes it was validated as.
pub struct SemanticReceiptArtifact {
    /// The built and validated receipt.
    pub receipt: NativeRouteSemanticReceipt,
    /// The canonical artifact bytes.
    pub bytes: Vec<u8>,
}

fn read_yaml(root: &Path, rel: &str) -> Result<Yaml, Box<dyn Error>> {
    let text = fs::read_to_string(root.join(rel))?;
    Ok(serde_yaml::from_str(&text)?)
}

fn yaml_str<'a>(value: &'a Yaml, pointer: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .as_str()
        .ok_or_else(|| format!("`{pointer}` is not a string").into())
}

/// The profile/configuration fingerprints the composer identity carries: the
/// declared release-probe reference profile and consolidation flags, the same
/// carriers the S03 threshold measurement binds.
pub fn load_composition_fingerprints(root: &Path) -> Result<(String, String), Box<dyn Error>> {
    let manifest = read_yaml(root, ACCEPTANCE_MANIFEST_REL)?;
    let lane = manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .ok_or("acceptance_manifest is missing external_holdout_contract.release_probe")?;
    let reference_profile = yaml_str(
        lane.get("reference_profile")
            .ok_or("release_probe.reference_profile is missing")?,
        "release_probe.reference_profile",
    )?;
    let flags = lane
        .get("reference_consolidation_flags")
        .and_then(Yaml::as_mapping)
        .ok_or("release_probe.reference_consolidation_flags is missing")?;
    let mut flag_pairs: Vec<String> = flags
        .iter()
        .map(|(key, value)| {
            Ok(format!(
                "{}={}",
                yaml_str(key, "reference_consolidation_flags key")?,
                yaml_str(value, "reference_consolidation_flags value")?
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    flag_pairs.sort();
    Ok((
        format!("profile={reference_profile}"),
        format!("consolidation={}", flag_pairs.join(",")),
    ))
}

/// One measured row, before it is turned into a receipt row.
pub struct RowMeasurement {
    /// The pinned route-corpus document id.
    pub document_id: String,
    /// `preserved_rich` or `holdout_structural`.
    pub profile_class: String,
    /// Bare 64-hex sha256 of the committed reference TEI.
    pub reference_sha256: String,
    /// `accepted` or `refused`.
    pub admitted_band_outcome: String,
    /// `admitted_band_unavailable:<reason>` when the band is unavailable.
    pub admitted_band_reason: String,
    /// `accepted` or `refused`.
    pub composer_outcome: String,
    /// The composer's typed refusal class when refused, else `None`.
    pub refusal_class: Option<String>,
    /// The composer's ledger when accepted, else `None`.
    pub ledger: Option<ReferenceLedgerView>,
}

impl RowMeasurement {
    fn into_measured(self) -> MeasuredSemanticDocument {
        MeasuredSemanticDocument {
            document_id: self.document_id,
            profile_class: self.profile_class,
            reference_sha256: self.reference_sha256,
            admitted_band_outcome: self.admitted_band_outcome,
            admitted_band_reason: self.admitted_band_reason,
            composer_outcome: self.composer_outcome,
            refusal_class: self.refusal_class,
            ledger: self.ledger,
        }
    }
}

fn unavailable_row(
    document_id: String,
    profile_class: String,
    reference_sha256: String,
    band_reason: &str,
) -> RowMeasurement {
    RowMeasurement {
        document_id,
        profile_class,
        reference_sha256,
        admitted_band_outcome: COMPOSER_REFUSED.to_owned(),
        admitted_band_reason: admitted_band_outcome_token(false, band_reason),
        composer_outcome: COMPOSER_REFUSED.to_owned(),
        refusal_class: Some(BAND_UNAVAILABLE_CLASS.to_owned()),
        ledger: None,
    }
}

/// Measure every reference TEI through the composer and return one row each.
pub fn measure_semantic_rows(root: &Path) -> Result<Vec<RowMeasurement>, Box<dyn Error>> {
    let layout = fs::read_to_string(root.join(LAYOUT_REL))?;
    let emission = fs::read(root.join(DEFAULT_EMISSION_ARTIFACT_REL))?;
    let expectation = derive_semantic_receipt_expectation(&layout, &emission)
        .map_err(|error| format!("expectation derivation: {error}"))?;
    let (profile_fingerprint, config_fingerprint) = load_composition_fingerprints(root)?;

    let mut rows = Vec::with_capacity(expectation.reference_documents.len());
    for binding in &expectation.reference_documents {
        let bytes = fs::read(root.join(&binding.path))?;
        let observed = sha256_prefixed(&bytes);
        let declared = format!("sha256:{}", binding.sha256);
        if observed != declared {
            return Err(format!(
                "reference TEI for {} drifted: observed {observed}, pinned {declared}",
                binding.id
            )
            .into());
        }
        let xml = String::from_utf8(bytes)
            .map_err(|error| format!("reference TEI for {} is not UTF-8: {error}", binding.id))?;
        let profile_class = profile_class_for_path(&binding.path);
        let identity_sha256 = binding.sha256.clone();

        let source = match measure_source(&binding.id, &identity_sha256, &xml) {
            Ok(source) => source,
            Err(_) => {
                rows.push(unavailable_row(
                    binding.id.clone(),
                    profile_class,
                    identity_sha256,
                    BAND_MEASURE_SOURCE_FAILED,
                ));
                continue;
            }
        };
        let admitted = match build_admitted_band(&source) {
            Ok(admitted) => admitted,
            Err(_) => {
                rows.push(unavailable_row(
                    binding.id.clone(),
                    profile_class,
                    identity_sha256,
                    BAND_BUILD_ADMITTED_BAND_FAILED,
                ));
                continue;
            }
        };
        match compose_native_document_typed(
            &profile_fingerprint,
            &config_fingerprint,
            &source,
            &admitted,
            &xml,
        ) {
            NativeCompositionMeasurement::Accepted { manifest, .. } => {
                let ledger = manifest.reference_links;
                rows.push(RowMeasurement {
                    document_id: binding.id.clone(),
                    profile_class,
                    reference_sha256: identity_sha256,
                    admitted_band_outcome: AB_ACCEPTED.to_owned(),
                    admitted_band_reason: AB_ACCEPTED.to_owned(),
                    composer_outcome: COMPOSER_ACCEPTED.to_owned(),
                    refusal_class: None,
                    ledger: Some(ReferenceLedgerView {
                        source_entries: ledger.source_entries as u64,
                        admitted_references: ledger.admitted_references as u64,
                        target_occurrences: ledger.target_occurrences as u64,
                        required_targets: ledger.required_targets as u64,
                        resolved_targets: ledger.resolved_targets as u64,
                        id_space: ledger.id_space as u64,
                    }),
                });
            }
            NativeCompositionMeasurement::Refused { class } => {
                rows.push(RowMeasurement {
                    document_id: binding.id.clone(),
                    profile_class,
                    reference_sha256: identity_sha256,
                    admitted_band_outcome: AB_ACCEPTED.to_owned(),
                    admitted_band_reason: AB_ACCEPTED.to_owned(),
                    composer_outcome: COMPOSER_REFUSED.to_owned(),
                    refusal_class: Some(class.to_owned()),
                    ledger: None,
                });
            }
        }
    }

    if rows.is_empty() {
        return Err("the route corpus binds no reference documents".into());
    }
    Ok(rows)
}

/// Build the semantic-correctness receipt from the committed carriers.
///
/// This is the single entry point the example runner and the contract test both
/// call, so the measured bytes and the committed artifact cannot diverge.
pub fn build_semantic_receipt_artifact(
    root: &Path,
) -> Result<SemanticReceiptArtifact, Box<dyn Error>> {
    let layout = fs::read_to_string(root.join(LAYOUT_REL))?;
    let emission = fs::read(root.join(DEFAULT_EMISSION_ARTIFACT_REL))?;
    let expectation = derive_semantic_receipt_expectation(&layout, &emission)
        .map_err(|error| format!("expectation derivation: {error}"))?;
    let rows = measure_semantic_rows(root)?;
    let measured: Vec<_> = rows
        .into_iter()
        .map(RowMeasurement::into_measured)
        .collect();
    let receipt = build_semantic_receipt(&expectation, &measured)
        .map_err(|error| format!("receipt build: {error}"))?;
    let bytes = receipt
        .artifact_json_bytes()
        .map_err(|error| format!("artifact bytes: {error}"))?;
    Ok(SemanticReceiptArtifact { receipt, bytes })
}
