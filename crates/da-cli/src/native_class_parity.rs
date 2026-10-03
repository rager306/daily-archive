//! M515-v2mvmc / S02: the shared class-parity projection.
//!
//! The two extraction thresholds the corpus acceptance runner must resolve
//! (`extraction.native_capability_recall`, micro and weakest stratum) are the
//! Java-relative reading of per-class conservation: for every measurement class,
//! `conserved / source_population`, aggregated as a micro occurrence count and as
//! the weakest class.  That projection lives in the 4,762-line runtime probe
//! example, which cannot be imported, so the corpus acceptance runner has no way
//! to reach it without a second hand copy that would drift from the
//! independently judged producer (MEM1972).
//!
//! This module is that projection, extracted once.  It sits beside the S01
//! shared composition surface ([`crate::native_composition_measurement`]) and
//! consumes its scanner, class enumeration and typed refusals, so the probe and
//! the acceptance runner read the same class-parity numbers instead of two
//! copies.  It is deliberately feature-free: the class enumeration and the
//! composed-emission carrier scans use only `da_parse` (an unconditional `da-cli`
//! dependency) and the shared S01 module, so it compiles in the default feature
//! set.
//!
//! A dropped class is a named refusal, never a silently shrunken denominator: an
//! emission whose class-carrying anchors (`teiHeader`, `listBibl`) are absent or
//! an enumerated profile set that does not cover every declared class both
//! refuse with [`CLASS_PARITY_CLASS_MISSING`].

use crate::native_composition_measurement::{
    AdmittedBand, CLASS_AUTHOR_AFFILIATION, CLASS_AUTHOR_LINE, CLASS_HEADER_EMAIL,
    CLASS_HEADER_IDS, CLASS_HEADER_TITLE, CLASS_ORDER, CLASS_REFERENCE_DOMAIN, ClassAdmission,
    MeasurementError, PROBE_CITATION_ROLES, ProbeSource, XmlScan, citation_class_wire_token,
    class_admissions, find_unique, scan_xml,
};
use da_parse::*;
use serde_json::Value;
use std::collections::BTreeSet;

/// The named refusal for an incomplete class set: an emission whose class
/// carriers cannot be enumerated, or an enumerated profile set that does not
/// cover every declared class.  A dropped class would silently shrink the
/// denominator, so it is refused by name instead.
pub const CLASS_PARITY_CLASS_MISSING: &str = "ClassParityClassMissing";

/// The named refusal for a typed composition rejection: the frozen reference's
/// admitted band did not compose.  Never a panic and never a fabricated zero.
pub const NATIVE_COMPOSITION_REFUSED: &str = "NativeCompositionRefused";

// ───────────────────────── shared rate arithmetic ──────────────────────────

/// `numerator / denominator`, or `None` when there is no denominator.  A null
/// denominator is `no_denominator` (MEM1702): never a vacuum `1.0`.
pub fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

/// Harmonic mean of two rates, or `None` unless both have a denominator.  A
/// genuine `0.0/0.0` (both rates exactly zero) is `0.0`, not `NaN`.
pub fn harmonic_mean(precision: Option<f64>, recall: Option<f64>) -> Option<f64> {
    match (precision, recall) {
        (Some(precision), Some(recall)) => Some(if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        }),
        _ => None,
    }
}

/// The integer-micros encoding of a rate (`round(1e6 * value)`), so no float is
/// ever serialized.
fn micros(value: f64) -> u64 {
    (value * 1_000_000.0).round() as u64
}

// ─────────────────────────── the class profile ─────────────────────────────

/// One class's measured profile: `TP = conserved`, `FN = lost`, `FP = fabricated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassProfile {
    pub class: &'static str,
    pub source_population: usize,
    pub admitted: usize,
    pub conserved: usize,
    pub lost: usize,
    pub fabricated: usize,
    /// Whether the class's carriers are also carried by the copied source
    /// subtree (read from DATA `class_parity.source_consumed_classes`, never a
    /// compiled-in list).  Such a class is *covered* by the union even when the
    /// admitted band is narrower, so its shortfall is a band-definition choice,
    /// not a fabricated loss.
    pub source_is_conserved: bool,
}

impl ClassProfile {
    /// The admitted/source coverage of the class, or `None` without a source
    /// population.
    pub fn admission_coverage(&self) -> Option<f64> {
        ratio(self.admitted, self.source_population)
    }

    /// Java-relative recall: `conserved / source_population`.  `None` (a
    /// `no_denominator` row) when the frozen reference declares no such carrier;
    /// never a vacuum `1.0` (MEM1702).
    pub fn recall_java(&self) -> Option<f64> {
        ratio(self.conserved, self.source_population)
    }

    /// `conserved / (conserved + fabricated)`, or `None` when the emission
    /// carries nothing at all for the class.
    pub fn precision(&self) -> Option<f64> {
        ratio(self.conserved, self.conserved + self.fabricated)
    }

    /// The harmonic mean of precision and Java-relative recall.
    pub fn f1(&self) -> Option<f64> {
        harmonic_mean(self.precision(), self.recall_java())
    }
}

/// Occurrence-counted conservation of text-valued carriers.
///
/// * `conserved` is `admitted` net of the occurrences the conserved source
///   subtrees already contribute: when the source's own carriers match the
///   admitted values, subtracting them makes a band that contributed nothing
///   read as `0` instead of a vacuous `1.0`.
/// * `fabricated` counts emitted band carriers matching neither an admitted value
///   nor a source value.
fn class_profile(
    class: &'static str,
    source_population: usize,
    admitted_values: &[String],
    emitted_carriers: &[String],
    source_carriers: &[String],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted_values
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
        source_is_conserved,
    }
}

/// Occurrence-counted conservation of binding-valued carriers (an owner element
/// plus a value: the author line's surname, or an affiliation key).
fn binding_profile(
    class: &'static str,
    source_population: usize,
    admitted_values: &[(String, String)],
    emitted_carriers: &[(String, String)],
    source_carriers: &[(String, String)],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let conserved = admitted_values.len().min(emitted_match);
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
        source_is_conserved,
    }
}

/// The emitted header's author-line carriers: `author-{ordinal}` -> surname.
fn emitted_author_lines(scan: &XmlScan) -> Vec<(String, String)> {
    let header = scan.unique("teiHeader");
    scan.descendants(header, "author")
        .into_iter()
        .filter_map(|author| {
            let id = scan.attr(author, "xml:id")?;
            let surname = scan
                .children(author, "surname")
                .first()
                .map(|&index| scan.text(index))
                .unwrap_or_default();
            Some((id.to_string(), surname))
        })
        .collect()
}

/// The emitted header's affiliation bindings: `author-{ordinal}` -> key.
fn emitted_affiliation_bindings(scan: &XmlScan) -> Vec<(String, String)> {
    let header = scan.unique("teiHeader");
    let mut bindings = Vec::new();
    for author in scan.descendants(header, "author") {
        let Some(owner) = scan.attr(author, "xml:id") else {
            continue;
        };
        for affiliation in scan.descendants(author, "affiliation") {
            if let Some(key) = scan.attr(affiliation, "key") {
                bindings.push((owner.to_string(), key.to_string()));
            }
        }
    }
    bindings
}

/// The emitted `listBibl` carriers of the measured citation roles.
fn emitted_citation_carriers(scan: &XmlScan) -> Vec<(String, String)> {
    let list = scan.unique("listBibl");
    scan.descendants(list, "note")
        .into_iter()
        .filter_map(|index| {
            let role = scan.attr(index, "role")?;
            let known = PROBE_CITATION_ROLES
                .iter()
                .any(|(candidate, _)| citation_field_role_token(*candidate) == role);
            known.then(|| (role.to_string(), scan.text(index)))
        })
        .collect()
}

/// Every class's measured profile for one document, from the emission bytes only.
///
/// The per-class admitted/source values come from the shared
/// [`class_admissions`] enumeration (one source of truth with the acceptance
/// runner); the source-consumed class set is read from DATA
/// (`release_probe.class_parity.source_consumed_classes`), never compiled in, so
/// a class the manifest marks as carried by the copied source subtree is measured
/// as conserved and its shortfall is not a fabricated loss.
///
/// A dropped class is refused by name ([`CLASS_PARITY_CLASS_MISSING`]) rather
/// than silently shrinking the denominator: the emission must carry unique
/// `teiHeader`/`listBibl` anchors, and the enumerated profile set must cover
/// every declared class.
pub fn measure_document_class_profiles(
    source: &ProbeSource,
    source_xml: &str,
    admitted: &AdmittedBand,
    emission_xml: &str,
    source_consumed_classes: &BTreeSet<String>,
) -> Result<Vec<ClassProfile>, MeasurementError> {
    let scan = scan_xml(emission_xml);
    if find_unique(&scan, "teiHeader").is_err() || find_unique(&scan, "listBibl").is_err() {
        return Err(MeasurementError::new(CLASS_PARITY_CLASS_MISSING));
    }
    let header = scan.unique("teiHeader");
    let list = scan.unique("listBibl");

    let emitted_titles = scan.text_list(&scan.descendants(header, "title"));
    let emitted_emails = scan.text_list(&scan.descendants(header, "email"));
    let emitted_ids = scan.text_list(&scan.descendants(header, "idno"));
    let emitted_reference_texts = scan.text_list(&scan.children(list, "biblStruct"));

    let emitted_lines = emitted_author_lines(&scan);
    let emitted_affiliations = emitted_affiliation_bindings(&scan);
    let emitted_citations = emitted_citation_carriers(&scan);

    let admissions = class_admissions(source, admitted, source_xml);
    let mut profiles = Vec::with_capacity(admissions.len());
    for admission in &admissions {
        let class = admission.class();
        let source_consumed = source_consumed_classes.contains(class);
        let profile = match admission {
            ClassAdmission::Binding {
                source_population,
                admitted,
                source_carriers,
                ..
            } => {
                let emitted = match class {
                    CLASS_AUTHOR_LINE => &emitted_lines,
                    CLASS_AUTHOR_AFFILIATION => &emitted_affiliations,
                    other => {
                        return Err(MeasurementError::new(format!(
                            "{CLASS_PARITY_CLASS_MISSING}: unbound binding class `{other}`"
                        )));
                    }
                };
                binding_profile(
                    class,
                    *source_population,
                    admitted,
                    emitted,
                    source_carriers,
                    source_consumed,
                )
            }
            ClassAdmission::Text {
                source_population,
                admitted,
                source_carriers,
                ..
            } => {
                let emitted: Vec<String> = if class == CLASS_HEADER_TITLE {
                    emitted_titles.clone()
                } else if class == CLASS_HEADER_EMAIL {
                    emitted_emails.clone()
                } else if class == CLASS_HEADER_IDS {
                    emitted_ids.clone()
                } else if class == CLASS_REFERENCE_DOMAIN {
                    emitted_reference_texts.clone()
                } else if let Some(token) = citation_class_wire_token(class) {
                    emitted_citations
                        .iter()
                        .filter(|(candidate, _)| candidate == token)
                        .map(|(_, value)| value.clone())
                        .collect()
                } else {
                    return Err(MeasurementError::new(format!(
                        "{CLASS_PARITY_CLASS_MISSING}: unbound text class `{class}`"
                    )));
                };
                class_profile(
                    class,
                    *source_population,
                    admitted,
                    &emitted,
                    source_carriers,
                    source_consumed,
                )
            }
        };
        profiles.push(profile);
    }
    if profiles.len() != CLASS_ORDER.len() {
        return Err(MeasurementError::new(CLASS_PARITY_CLASS_MISSING));
    }
    Ok(profiles)
}

// ─────────────────────────── composition harness ───────────────────────────

/// Compose one document's admitted band with the single SUT this module reaches,
/// returning the emission XML.
///
/// A typed composition refusal becomes a named [`MeasurementError`]
/// ([`NATIVE_COMPOSITION_REFUSED`]), never a panic: a document the composer
/// refuses is not measured, and its refusal is carried, not swallowed.
pub fn compose_native_document(
    profile_fingerprint: &str,
    config_fingerprint: &str,
    source: &ProbeSource,
    admitted: &AdmittedBand,
    source_tei_xml: &str,
) -> Result<String, MeasurementError> {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            source_tei_xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, profile_fingerprint.to_string()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: profile_fingerprint.to_string(),
            config_fingerprint: config_fingerprint.to_string(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: probe_admissions(&source.sha256),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => {
            String::from_utf8(candidate.envelope.raw_tei().to_vec()).map_err(|error| {
                MeasurementError::new(format!(
                    "{NATIVE_COMPOSITION_REFUSED}: emitted document is not UTF-8 ({error})"
                ))
            })
        }
        NativeFulltextCompositionOutcome::Rejected { error, .. } => Err(MeasurementError::new(
            format!("{NATIVE_COMPOSITION_REFUSED}: {error:?}"),
        )),
    }
}

/// Header and references admitted, every other component typed unsupported or
/// not observed.
pub fn probe_admissions(source_sha256: &str) -> Vec<FulltextAdmissionReceipt> {
    let receipt = |component: FulltextComponent, disposition: AdmissionDisposition| {
        FulltextAdmissionReceipt {
            component,
            disposition,
            producer: "frozen-external-holdout-reference".into(),
            evidence_state: if disposition == AdmissionDisposition::Passed {
                "passing"
            } else {
                "unsupported"
            }
            .into(),
            schema_version: "native-composition.outcome/v1".into(),
            source_sha256: source_sha256.to_string(),
            artifact_sha256: String::new(),
            fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
        }
    };
    vec![
        receipt(FulltextComponent::Header, AdmissionDisposition::Passed),
        receipt(FulltextComponent::References, AdmissionDisposition::Passed),
        receipt(
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::InvalidEvidence,
        ),
        receipt(FulltextComponent::Figure, AdmissionDisposition::Unsupported),
        receipt(FulltextComponent::Table, AdmissionDisposition::NotObserved),
        receipt(
            FulltextComponent::Formula,
            AdmissionDisposition::Unsupported,
        ),
        receipt(
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
        ),
    ]
}

// ─────────────────────────── the parity aggregate ──────────────────────────

/// The class-parity aggregate over a set of measured profiles.
///
/// `native_tp`/`native_denominator` accumulate only for the classes the native
/// composer actually produces (the source-consumed classes are excluded from the
/// micro recall); `per_class` aggregates the conserved count and source
/// population per class token, so the weakest class is a class-level rate, not a
/// document-level one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassParityCounts {
    pub native_tp: usize,
    pub native_denominator: usize,
    /// `(class, conserved, source_population)` per class token, first-seen order.
    pub per_class: Vec<(String, usize, usize)>,
}

impl ClassParityCounts {
    /// Reproduce the probe's `ClassParity::of` accumulation from the shared
    /// [`ClassProfile`] rows.
    pub fn from_profiles(profiles: &[ClassProfile]) -> ClassParityCounts {
        let mut counts = ClassParityCounts::default();
        for profile in profiles {
            if !profile.source_is_conserved {
                counts.native_tp += profile.conserved;
                counts.native_denominator += profile.source_population;
            }
            match counts
                .per_class
                .iter_mut()
                .find(|(class, _, _)| class.as_str() == profile.class)
            {
                Some((_, conserved, source_population)) => {
                    *conserved += profile.conserved;
                    *source_population += profile.source_population;
                }
                None => counts.per_class.push((
                    profile.class.to_string(),
                    profile.conserved,
                    profile.source_population,
                )),
            }
        }
        counts
    }

    /// The micro occurrence recall `native_tp / native_denominator`, or `None`
    /// when no native class declares a carrier.
    pub fn native_capability_recall(&self) -> Option<f64> {
        ratio(self.native_tp, self.native_denominator)
    }

    /// The micro occurrence recall frozen as integer micros, or `None` without a
    /// denominator.
    pub fn native_capability_recall_micros(&self) -> Option<u64> {
        self.native_capability_recall().map(micros)
    }

    /// The native capability recall encoded at the *declared* fraction
    /// denominator (`round(recall * denominator)`), or `None` without a
    /// denominator.  The frozen micros derivation is `recall_micros / 1e6`, so
    /// the denominator is read by pointer from the pins rather than restated
    /// here.
    pub fn native_capability_recall_micros_at(&self, denominator: u64) -> Option<u64> {
        self.native_capability_recall()
            .map(|recall| (recall * denominator as f64).round() as u64)
    }

    /// The weakest class by `conserved / source_population`, or `None` when no
    /// class declares a carrier.  Ties keep the first-seen class, matching the
    /// pinned reference aggregation.
    pub fn weakest_class(&self) -> Option<(&str, usize, usize)> {
        let mut weakest: Option<(&str, usize, usize)> = None;
        for (class, conserved, source_population) in &self.per_class {
            if *source_population == 0 {
                continue;
            }
            let better = match weakest {
                None => true,
                Some((_, weakest_conserved, weakest_source)) => {
                    (*conserved as u128) * (weakest_source as u128)
                        < (weakest_conserved as u128) * (*source_population as u128)
                }
            };
            if better {
                weakest = Some((class.as_str(), *conserved, *source_population));
            }
        }
        weakest
    }
}

// ─────────────────────── the declared extraction projections ────────────────

/// The named refusal for a frozen extraction derivation this module does not
/// implement: a changed derivation must refuse by name rather than be silently
/// skipped into a gapped threshold.
pub const CLASS_PARITY_PROJECTION_UNKNOWN: &str = "ClassParityProjectionUnknown";

/// Which projection of the class-parity aggregate a frozen extraction threshold
/// declares.  The two extraction thresholds share one metric id and differ only
/// by this projection, so a row that does not name its projection could be
/// substituted by the wrong stratum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassParityProjection {
    /// The micro occurrence recall encoded as integer micros at the declared
    /// denominator (`native_capability_recall` over one denominator).
    MicroOccurrence,
    /// The weakest class by `conserved / source_population`.
    WeakestStratum,
}

/// One frozen extraction threshold's declared class-parity projection, read by
/// pointer from the pins' `derivation` (never a restated literal).  The stratum
/// and metric id are the frozen DATA tokens the receipt row must carry; the
/// reference denominator is the micros base the micro projection encodes at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionThresholdProjection {
    pub metric_id: String,
    pub stratum: String,
    pub projection: ClassParityProjection,
    pub reference_denominator: u64,
}

/// Read the class-parity projections the pins declare, by pointer.
///
/// The pins name each threshold's exact derivation, so the runner executes the
/// declared projection instead of guessing which stratum is which.  An
/// unrecognized class-parity derivation (one that names `class_parity` or
/// `class_profiles` but matches neither known projection) is refused by name; a
/// non-class-parity pin (the preservation threshold's `admitted /
/// source_population`) is skipped.
///
/// `pins_text` is the committed `absorption_progress_reference_pins.json` bytes.
pub fn load_class_parity_projections(
    pins_text: &str,
) -> Result<Vec<ExtractionThresholdProjection>, MeasurementError> {
    let value: Value = serde_json::from_str(pins_text).map_err(|error| {
        MeasurementError::new(format!(
            "{CLASS_PARITY_PROJECTION_UNKNOWN}: pins are not valid JSON ({error})"
        ))
    })?;
    let values = value
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            MeasurementError::new(format!(
                "{CLASS_PARITY_PROJECTION_UNKNOWN}: pins carry no `values` array"
            ))
        })?;
    let mut projections = Vec::new();
    for entry in values {
        let derivation = entry
            .get("derivation")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let projection = if derivation.contains("native_capability_recall_micros") {
            ClassParityProjection::MicroOccurrence
        } else if derivation.contains("minimum")
            && derivation.contains("conserved/source_population")
        {
            ClassParityProjection::WeakestStratum
        } else if derivation.contains("class_parity") || derivation.contains("class_profiles") {
            return Err(MeasurementError::new(format!(
                "{CLASS_PARITY_PROJECTION_UNKNOWN}: unrecognized class-parity derivation `{derivation}`"
            )));
        } else {
            continue;
        };
        let metric_id = entry
            .get("metric_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MeasurementError::new(format!(
                    "{CLASS_PARITY_PROJECTION_UNKNOWN}: a class-parity derivation carries no metric_id"
                ))
            })?
            .to_string();
        let stratum = entry
            .get("stratum")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                MeasurementError::new(format!(
                    "{CLASS_PARITY_PROJECTION_UNKNOWN}: a class-parity derivation carries no stratum"
                ))
            })?
            .to_string();
        let reference_denominator = entry
            .get("fraction")
            .and_then(|fraction| fraction.get("denominator"))
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                MeasurementError::new(format!(
                    "{CLASS_PARITY_PROJECTION_UNKNOWN}: a class-parity derivation carries no fraction denominator"
                ))
            })?;
        projections.push(ExtractionThresholdProjection {
            metric_id,
            stratum,
            projection,
            reference_denominator,
        });
    }
    Ok(projections)
}
