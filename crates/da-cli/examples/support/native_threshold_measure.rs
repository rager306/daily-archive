//! M516-th0oby / S03 / T03 — measurement-plane helper for the threshold
//! candidates.
//!
//! This module folds the expanded native route corpus's reference TEI through the
//! *shared* composition and class-parity projections
//! (`da_cli::native_composition_measurement` + `da_cli::native_class_parity`, the
//! same modules `native_corpus_acceptance` drives) to produce the three frozen
//! absorption-progress threshold candidates.
//!
//! Like `native_compose.rs`, it deliberately lives outside `crates/da-cli/src`:
//! it names the quarantined composer's identifiers, and the one call site that
//! must compose a document to *measure* a threshold stays in the example/test
//! plane (D381). It is `#[path]`-shared by the example runner and the contract
//! test, so both measure the identical path.
//!
//! Nothing is hand-typed: every reference TEI is sha256-verified against the
//! DATA block before it is measured, and the metric set is resolved from the
//! frozen pins' own derivations.

#[path = "native_compose.rs"]
mod native_compose;

use self::native_compose::compose_native_document;
use da_cli::native_class_parity::{
    ClassParityCounts, ClassParityProjection, load_class_parity_projections,
    measure_document_class_profiles,
};
use da_cli::native_composition_measurement::{
    build_admitted_band, class_admissions, measure_source,
};
use da_cli::native_family_receipt::sha256_prefixed;
use da_cli::native_route_threshold_candidates::{
    CandidateValue, Fraction, MeasuredMetric, PRESERVATION_METRIC_ID,
    ThresholdCandidatesExpectation, ThresholdCandidatesReceipt, build_threshold_candidates_receipt,
    corpus_document_ids, derive_threshold_candidates_expectation, reference_document_bindings,
};
use serde_yaml::Value as Yaml;
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Repository-relative carriers the measurement reads.
const LAYOUT_REL: &str = "data/parse/layout_bakeoff.yaml";
const SCHEMA_REL: &str = "data/parse/absorption_progress.schema.yaml";
const HOLDOUT_REL: &str = "data/parse/holdout_manifests.yaml";
const ACCEPTANCE_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const REFERENCE_PINS_REL: &str = "artifacts/parse_compat/absorption_progress_reference_pins.json";

/// One reference TEI bound to a route-corpus document id.
pub struct ReferenceDocument {
    /// Route-corpus document id.
    pub id: String,
    /// Repository-relative reference TEI path.
    pub path: String,
    /// sha256 digest (bare or `sha256:`-prefixed) of the reference TEI bytes.
    pub sha256: String,
}

/// The class-parity profile/configuration fingerprints and the source-consumed
/// class set, read from the frozen acceptance manifest's `release_probe` block.
pub struct MeasurementInputs {
    /// Reference TEI bound per route-corpus document.
    pub reference_documents: Vec<ReferenceDocument>,
    /// Parse profile fingerprint.
    pub profile_fingerprint: String,
    /// Parse configuration fingerprint.
    pub config_fingerprint: String,
    /// The class set the source subtree carries (measured as conserved).
    pub source_consumed_classes: BTreeSet<String>,
    /// The committed reference pins bytes.
    pub pins_text: String,
}

/// The fully derived artifact plus the canonical bytes it was validated as.
pub struct ThresholdCandidatesArtifact {
    /// The built and validated receipt.
    pub receipt: ThresholdCandidatesReceipt,
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

/// Read the class-parity profile/configuration fingerprints and the
/// source-consumed class set from the frozen acceptance manifest.
pub fn load_class_parity_inputs(
    root: &Path,
) -> Result<(String, String, BTreeSet<String>), Box<dyn Error>> {
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
    let source_consumed_classes: BTreeSet<String> = lane
        .get("class_parity")
        .and_then(|parity| parity.get("source_consumed_classes"))
        .and_then(Yaml::as_sequence)
        .ok_or("release_probe.class_parity.source_consumed_classes is missing")?
        .iter()
        .map(|class| yaml_str(class, "source_consumed_classes entry").map(str::to_owned))
        .collect::<Result<BTreeSet<_>, Box<dyn Error>>>()?;
    if source_consumed_classes.is_empty() {
        return Err("release_probe.class_parity.source_consumed_classes is empty".into());
    }
    Ok((
        format!("profile={reference_profile}"),
        format!("consolidation={}", flag_pairs.join(",")),
        source_consumed_classes,
    ))
}

/// Read the reference-TEI bindings and the class-parity inputs from DATA.
pub fn load_measurement_inputs(
    root: &Path,
    layout_yaml: &str,
) -> Result<MeasurementInputs, Box<dyn Error>> {
    let bindings = reference_document_bindings(layout_yaml)
        .map_err(|error| format!("reference bindings: {error}"))?;
    let reference_documents = bindings
        .into_iter()
        .map(|binding| ReferenceDocument {
            id: binding.id,
            path: binding.path,
            sha256: binding.sha256,
        })
        .collect();
    let (profile_fingerprint, config_fingerprint, source_consumed_classes) =
        load_class_parity_inputs(root)?;
    let pins_text = fs::read_to_string(root.join(REFERENCE_PINS_REL))?;
    Ok(MeasurementInputs {
        reference_documents,
        profile_fingerprint,
        config_fingerprint,
        source_consumed_classes,
        pins_text,
    })
}

/// Measure the three candidates over the route corpus's reference TEI.
///
/// Every reference TEI is sha256-verified before it is folded; a drift refuses
/// by name rather than shrinking the denominator. The preservation aggregate is
/// the sum of the shared scanner's `admitted`/`source_population` rows; the two
/// extraction strata are projected from the shared class-parity aggregate. The
/// candidate set is the pins' own derivations (never a restated list).
pub fn measure_threshold_candidates(
    root: &Path,
    inputs: &MeasurementInputs,
) -> Result<Vec<MeasuredMetric>, Box<dyn Error>> {
    let projections = load_class_parity_projections(&inputs.pins_text)
        .map_err(|error| format!("class-parity projections: {error}"))?;
    if inputs.reference_documents.is_empty() {
        return Err("the route corpus binds no reference documents".into());
    }

    let mut preservation_admitted = 0u64;
    let mut preservation_source = 0u64;
    let mut class_profiles = Vec::new();
    let mut measured_documents = 0u64;

    for document in &inputs.reference_documents {
        let bytes = fs::read(root.join(&document.path))?;
        let observed = sha256_prefixed(&bytes);
        let declared = if document.sha256.starts_with("sha256:") {
            document.sha256.clone()
        } else {
            format!("sha256:{}", document.sha256)
        };
        if observed != declared {
            return Err(format!(
                "reference TEI for {} drifted: observed {observed}, pinned {declared}",
                document.id
            )
            .into());
        }
        let xml = String::from_utf8(bytes)
            .map_err(|error| format!("reference TEI for {} is not UTF-8: {error}", document.id))?;
        let identity_sha256 = declared
            .strip_prefix("sha256:")
            .unwrap_or(&declared)
            .to_string();

        let source = measure_source(&document.id, &identity_sha256, &xml)
            .map_err(|error| format!("measure_source {}: {error}", document.id))?;
        let admitted = build_admitted_band(&source)
            .map_err(|error| format!("build_admitted_band {}: {error}", document.id))?;
        for admission in class_admissions(&source, &admitted, &xml) {
            preservation_admitted += admission.admitted_len() as u64;
            preservation_source += admission.source_population() as u64;
        }
        let emission = compose_native_document(
            &inputs.profile_fingerprint,
            &inputs.config_fingerprint,
            &source,
            &admitted,
            &xml,
        )
        .map_err(|error| format!("compose_native_document {}: {error}", document.id))?;
        let document_profiles = measure_document_class_profiles(
            &source,
            &xml,
            &admitted,
            &emission,
            &inputs.source_consumed_classes,
        )
        .map_err(|error| format!("measure_document_class_profiles {}: {error}", document.id))?;
        class_profiles.extend(document_profiles);
        measured_documents += 1;
    }

    if measured_documents == 0 {
        return Err("no reference documents were measured".into());
    }
    if preservation_source == 0 {
        return Err("the preservation denominator is empty".into());
    }

    let class_parity = ClassParityCounts::from_profiles(&class_profiles);
    let mut measured = Vec::new();
    measured.push(MeasuredMetric {
        metric_id: PRESERVATION_METRIC_ID.to_owned(),
        // The preservation metric id is carried by one frozen threshold, so the
        // measured row legitimately omits the stratum.
        stratum: None,
        candidate: CandidateValue {
            fraction: Fraction {
                numerator: preservation_admitted,
                denominator: preservation_source,
            },
            value: preservation_admitted as f64 / preservation_source as f64,
            denominator: preservation_source,
        },
    });

    for projection in &projections {
        match projection.projection {
            ClassParityProjection::MicroOccurrence => {
                let numerator = class_parity
                    .native_capability_recall_micros_at(projection.reference_denominator)
                    .ok_or_else(|| {
                        format!(
                            "the class-parity micro stratum `{}` has an empty denominator",
                            projection.stratum
                        )
                    })?;
                measured.push(MeasuredMetric {
                    metric_id: projection.metric_id.clone(),
                    stratum: Some(projection.stratum.clone()),
                    candidate: CandidateValue {
                        fraction: Fraction {
                            numerator,
                            denominator: projection.reference_denominator,
                        },
                        value: numerator as f64 / projection.reference_denominator as f64,
                        denominator: projection.reference_denominator,
                    },
                });
            }
            ClassParityProjection::WeakestStratum => {
                let (_class, conserved, source_population) =
                    class_parity.weakest_class().ok_or_else(|| {
                        format!(
                            "the class-parity weakest stratum `{}` measured no class",
                            projection.stratum
                        )
                    })?;
                if source_population == 0 {
                    return Err(format!(
                        "the class-parity weakest stratum `{}` has an empty denominator",
                        projection.stratum
                    )
                    .into());
                }
                measured.push(MeasuredMetric {
                    metric_id: projection.metric_id.clone(),
                    stratum: Some(projection.stratum.clone()),
                    candidate: CandidateValue {
                        fraction: Fraction {
                            numerator: conserved as u64,
                            denominator: source_population as u64,
                        },
                        value: conserved as f64 / source_population as f64,
                        denominator: source_population as u64,
                    },
                });
            }
        }
    }

    Ok(measured)
}

/// The comparability rationale for the measured corpus, derived (never forced).
pub fn comparability_rationale(expectation: &ThresholdCandidatesExpectation) -> String {
    if expectation.comparability_verdict
        == da_cli::native_route_threshold_candidates::COMPARABILITY_MATCHED
    {
        format!(
            "the measured corpus identity `{}` ({} documents) matches the frozen reference \
             population `{}` ({} documents), so the candidate is comparable to the pin",
            expectation.corpus_id,
            expectation.corpus_document_count,
            expectation.reference_population.id,
            expectation.reference_population.document_count,
        )
    } else {
        format!(
            "the measured corpus is the `{}` native PDF-to-TEI route corpus ({} documents / {} \
             pages), while the frozen reference pin population is `{}` ({} documents); the corpus \
             identities do not match, so the candidate is not comparable to the pin and no \
             acceptance verdict is claimed",
            expectation.corpus_id,
            expectation.corpus_document_count,
            expectation.corpus_page_denominator,
            expectation.reference_population.id,
            expectation.reference_population.document_count,
        )
    }
}

/// Build the threshold-candidate artifact from the committed carriers.
///
/// This is the single entry point the example runner and the contract test both
/// call, so the measured bytes and the committed artifact cannot diverge.
pub fn build_threshold_candidates_artifact(
    root: &Path,
) -> Result<ThresholdCandidatesArtifact, Box<dyn Error>> {
    let pins = fs::read(root.join(REFERENCE_PINS_REL))?;
    let layout = fs::read_to_string(root.join(LAYOUT_REL))?;
    let schema = fs::read_to_string(root.join(SCHEMA_REL))?;
    let holdout = fs::read_to_string(root.join(HOLDOUT_REL))?;
    let expectation = derive_threshold_candidates_expectation(&pins, &layout, &schema, &holdout)
        .map_err(|error| format!("expectation derivation: {error}"))?;

    // The reference bindings must cover the declared corpus exactly; the id sets
    // are recomputed from DATA so a shrunken or over-bound set refuses here.
    let corpus_ids =
        corpus_document_ids(&layout).map_err(|error| format!("route corpus ids: {error}"))?;
    let inputs = load_measurement_inputs(root, &layout)?;
    let bound_ids: BTreeSet<&str> = inputs
        .reference_documents
        .iter()
        .map(|document| document.id.as_str())
        .collect();
    if corpus_ids.len() != bound_ids.len()
        || !corpus_ids.iter().all(|id| bound_ids.contains(id.as_str()))
    {
        return Err("the reference bindings do not cover the route corpus exactly".into());
    }

    let measured = measure_threshold_candidates(root, &inputs)?;
    let rationale = comparability_rationale(&expectation);
    let receipt = build_threshold_candidates_receipt(&expectation, &measured, &rationale)
        .map_err(|error| format!("receipt build: {error}"))?;
    let bytes = receipt
        .artifact_json_bytes()
        .map_err(|error| format!("artifact bytes: {error}"))?;

    Ok(ThresholdCandidatesArtifact { receipt, bytes })
}
