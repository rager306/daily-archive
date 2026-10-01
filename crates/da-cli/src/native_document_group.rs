//! S05/T05 (M514-1lss2s) — OCR/scanned vs digital document groups, and the
//! required-family disposition vocabulary that structurally cannot say
//! `out_of_scope`.
//!
//! S05 evaluates body text, sections, reading order, visual families and
//! callouts on the native plane. Two facts about that evaluation were untyped
//! before this module:
//!
//! 1. the corpus was never *partitioned*, so a required family could not be
//!    stratified by "does this document carry a text layer at all?"; and
//! 2. the visual-family disposition vocabulary mixed "unsupported" with
//!    "not observed" with no mandatory `unmet` arm — so *moving a required
//!    family into `out_of_scope`* was indistinguishable from an honest refusal.
//!
//! This module is the typed vocabulary for both. It is pure `std`/`serde` and
//! reads [`da_domain::pdf_classification::PdfType`] (a domain type available in
//! the default feature set), so the module and its contract test compile
//! without `pdf-layout` and without `pdf-classifier`. It owns no route, no
//! classifier, no filesystem access and no graph write.
//!
//! ## Partition totality
//!
//! [`DocumentGroupPartition::assemble`] is total over the declared corpus: every
//! declared document carries **exactly one** [`DocumentGroupEvidence`]. A
//! document whose PDF bytes were absent is *not* folded into
//! [`DocumentGroup::Unknown`] — it carries a typed [`DocumentBytesAbsentReason`]
//! and no group at all, because "unknown" (the classifier ran and could not
//! decide) and "absent" (there was nothing to classify) are different facts
//! with different consequences. That is why [`DocumentGroupEvidence::group`] is
//! an `Option`: the absence is structural, not a sentinel value a consumer can
//! mistake for a measurement.
//!
//! ## `out_of_scope` is not a variant
//!
//! [`RequiredFamilyDisposition`] has exactly three arms: `measured`,
//! `unmet{reason}` and `not_observed`. `out_of_scope` is *not* one of them.
//! Serde therefore rejects the token, and
//! [`RequiredFamilyDisposition::parse`] refuses it **by name**
//! ([`DocumentGroupError::RequiredFamilyOutOfScope`]), so a downstream receipt
//! validator can report the exact mutation instead of a generic parse error.
//! [`FORBIDDEN_REQUIRED_FAMILY_TOKEN`] is exported for that validator and for
//! the governor's negative fixtures.

use da_domain::pdf_classification::PdfType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema version of the emitted partition, when one is serialized.
pub const DOCUMENT_GROUP_SCHEMA_VERSION: &str = "native-document-group.v1";

/// Vocabulary names carried by typed refusals, so a refusal says which closed
/// set it was checking against.
pub const DOCUMENT_GROUP_VOCABULARY_NAME: &str = "document_group";
/// See [`DOCUMENT_GROUP_VOCABULARY_NAME`].
pub const DOCUMENT_GROUP_SOURCE_VOCABULARY_NAME: &str = "document_group_source";
/// See [`DOCUMENT_GROUP_VOCABULARY_NAME`].
pub const DOCUMENT_GROUP_PARTITION_STATE_VOCABULARY_NAME: &str = "document_group_partition_state";
/// See [`DOCUMENT_GROUP_VOCABULARY_NAME`].
pub const REQUIRED_FAMILY_DISPOSITION_VOCABULARY_NAME: &str = "required_family_disposition";
/// See [`DOCUMENT_GROUP_VOCABULARY_NAME`].
pub const REQUIRED_FAMILY_UNMET_VOCABULARY_NAME: &str = "required_family_unmet_reason";
/// See [`DOCUMENT_GROUP_VOCABULARY_NAME`].
pub const DOCUMENT_BYTES_ABSENT_VOCABULARY_NAME: &str = "document_bytes_absent_reason";

/// The token that must never appear in the required-family vocabulary. It is
/// deliberately not a variant of [`RequiredFamilyDisposition`]; naming it here
/// is what turns "it cannot be represented" into "it is refused by name".
pub const FORBIDDEN_REQUIRED_FAMILY_TOKEN: &str = "out_of_scope";

/// Closed [`DocumentGroup`] dictionary, in canonical order.
pub const DOCUMENT_GROUP_VOCABULARY: [&str; 4] =
    ["digital_text", "scanned_image", "mixed", "unknown"];
/// Closed [`DocumentGroupSource`] dictionary, in canonical order.
pub const DOCUMENT_GROUP_SOURCE_VOCABULARY: [&str; 2] = ["measured", "bytes_absent"];
/// Closed [`DocumentGroupPartitionState`] dictionary, in canonical order.
pub const DOCUMENT_GROUP_PARTITION_STATE_VOCABULARY: [&str; 2] =
    ["partitioned", "empty_declared_corpus"];
/// Closed [`RequiredFamilyDisposition`] dictionary, in canonical order. It has
/// no `out_of_scope` member.
pub const REQUIRED_FAMILY_DISPOSITION_VOCABULARY: [&str; 3] = ["measured", "unmet", "not_observed"];
/// Closed [`RequiredFamilyUnmetReason`] dictionary, in canonical order. It has
/// no `out_of_scope` member.
pub const REQUIRED_FAMILY_UNMET_VOCABULARY: [&str; 4] = [
    "no_text_layer",
    "native_lane_unsupported",
    "oracle_absent",
    "oracle_vacuous",
];
/// Closed [`DocumentBytesAbsentReason`] dictionary, in canonical order.
pub const DOCUMENT_BYTES_ABSENT_VOCABULARY: [&str; 3] = [
    "pdf_bytes_missing",
    "pdf_read_failed",
    "classifier_unavailable",
];

// ───────────────────────────── groups ─────────────────────────────

/// Closed OCR/scanned vs digital partition.
///
/// The four arms are the whole vocabulary. `scanned_image` is the arm that
/// carries no text layer (a scanned document and an image-only document both
/// land here: see [`DocumentGroup::from_pdf_type`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentGroup {
    /// Digital-born document with an extractable text layer.
    DigitalText,
    /// Image-only document: scanned or composed of raster images, no text layer.
    ScannedImage,
    /// Some pages carry text, some do not.
    Mixed,
    /// The classifier ran and could not decide. This is *not* absence.
    Unknown,
}

impl DocumentGroup {
    /// The serialized token for this arm. Kept in lockstep with
    /// [`DOCUMENT_GROUP_VOCABULARY`] by test.
    pub fn token(&self) -> &'static str {
        match self {
            DocumentGroup::DigitalText => "digital_text",
            DocumentGroup::ScannedImage => "scanned_image",
            DocumentGroup::Mixed => "mixed",
            DocumentGroup::Unknown => "unknown",
        }
    }

    /// Fail-closed parse of the closed dictionary.
    pub fn from_token(token: &str) -> Result<Self, DocumentGroupError> {
        match token {
            "digital_text" => Ok(DocumentGroup::DigitalText),
            "scanned_image" => Ok(DocumentGroup::ScannedImage),
            "mixed" => Ok(DocumentGroup::Mixed),
            "unknown" => Ok(DocumentGroup::Unknown),
            other => Err(DocumentGroupError::UnknownVocabularyToken {
                vocabulary: DOCUMENT_GROUP_VOCABULARY_NAME,
                token: other.to_string(),
            }),
        }
    }

    /// The **single** `PdfType` → `DocumentGroup` mapping.
    ///
    /// The mapping is total and lives only here. `Scanned` and `Image` both map
    /// to [`DocumentGroup::ScannedImage`] because, for the purpose this
    /// partition exists — whether a required text-bearing family can be
    /// measured at all — a scan and an image-only document are the same fact:
    /// no text layer. Folding them is a required-family necessity, not a claim
    /// that the two provenance classes are identical.
    pub fn from_pdf_type(pdf_type: &PdfType) -> Self {
        match pdf_type {
            PdfType::Text => DocumentGroup::DigitalText,
            PdfType::Scanned => DocumentGroup::ScannedImage,
            PdfType::Image => DocumentGroup::ScannedImage,
            PdfType::Mixed => DocumentGroup::Mixed,
            PdfType::Unknown => DocumentGroup::Unknown,
        }
    }

    /// The typed reason a group cannot carry a text-bearing required family, or
    /// `None` when it can. Only [`DocumentGroup::ScannedImage`] has no text
    /// layer; `unknown` is unresolved, not text-free, and is therefore not
    /// forced to `unmet`.
    pub fn text_layer_absence(&self) -> Option<RequiredFamilyUnmetReason> {
        match self {
            DocumentGroup::ScannedImage => Some(RequiredFamilyUnmetReason::NoTextLayer),
            DocumentGroup::DigitalText | DocumentGroup::Mixed | DocumentGroup::Unknown => None,
        }
    }

    /// The disposition this group *forces* on a required family, independent of
    /// any measurement. `Some(unmet{no_text_layer})` for a scanned document;
    /// `None` for every group that can, in principle, be measured.
    pub fn forced_required_family_disposition(&self) -> Option<RequiredFamilyDisposition> {
        self.text_layer_absence()
            .map(RequiredFamilyDisposition::Unmet)
    }
}

// ───────────────────────── group evidence ─────────────────────────

/// Closed reason a document's bytes were absent, so no group could be assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentBytesAbsentReason {
    /// The declared PDF path carries no bytes.
    PdfBytesMissing,
    /// The declared PDF exists but could not be read.
    PdfReadFailed,
    /// The bytes are present but no classifier verdict was produced.
    ClassifierUnavailable,
}

impl DocumentBytesAbsentReason {
    /// The serialized token for this arm.
    pub fn token(&self) -> &'static str {
        match self {
            DocumentBytesAbsentReason::PdfBytesMissing => "pdf_bytes_missing",
            DocumentBytesAbsentReason::PdfReadFailed => "pdf_read_failed",
            DocumentBytesAbsentReason::ClassifierUnavailable => "classifier_unavailable",
        }
    }

    /// Fail-closed parse of the closed dictionary.
    pub fn from_token(token: &str) -> Result<Self, DocumentGroupError> {
        match token {
            "pdf_bytes_missing" => Ok(DocumentBytesAbsentReason::PdfBytesMissing),
            "pdf_read_failed" => Ok(DocumentBytesAbsentReason::PdfReadFailed),
            "classifier_unavailable" => Ok(DocumentBytesAbsentReason::ClassifierUnavailable),
            other => Err(DocumentGroupError::UnknownVocabularyToken {
                vocabulary: DOCUMENT_BYTES_ABSENT_VOCABULARY_NAME,
                token: other.to_string(),
            }),
        }
    }
}

/// The positive measurement behind a group: which classifier said what about
/// which bytes. Every identity here is repository-relative provenance, never an
/// absolute host path (redaction is asserted downstream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentGroupMeasured {
    /// Raw classifier verdict. [`DocumentGroupEvidence::group`] is its mapping
    /// through [`DocumentGroup::from_pdf_type`]; the two are cross-checked when
    /// the partition is assembled.
    pub pdf_type: PdfType,
    /// Identity of the classifier that produced `pdf_type`.
    pub classifier_id: String,
    /// Version of that classifier.
    pub classifier_version: String,
    /// sha256 of the classified PDF bytes.
    pub pdf_sha256: String,
}

/// Typed absence: the document was declared, but there was nothing to classify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentGroupBytesAbsent {
    /// Why the bytes were absent.
    pub reason: DocumentBytesAbsentReason,
}

/// Where a document's group came from. `Measured` carries a verdict;
/// `BytesAbsent` carries a reason and no verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentGroupSource {
    /// A classifier produced a verdict over present bytes.
    Measured(DocumentGroupMeasured),
    /// No verdict exists because the bytes were absent.
    BytesAbsent(DocumentGroupBytesAbsent),
}

/// One declared document's group assignment.
///
/// `group` is `Some` exactly when the source is `Measured`. The absence of the
/// bytes is therefore structural: a `BytesAbsent` row has no group field and
/// cannot be read as [`DocumentGroup::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentGroupEvidence {
    /// Declared document identity (corpus-relative id, not a path).
    pub document_id: String,
    /// The assigned group, or `None` for a typed absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<DocumentGroup>,
    /// Provenance of the assignment.
    pub source: DocumentGroupSource,
}

impl DocumentGroupEvidence {
    /// Build a measured row. The group is derived from `pdf_type` here, so an
    /// inconsistent pair cannot be constructed through this constructor.
    pub fn measured(
        document_id: impl Into<String>,
        pdf_type: PdfType,
        classifier_id: impl Into<String>,
        classifier_version: impl Into<String>,
        pdf_sha256: impl Into<String>,
    ) -> Self {
        Self {
            document_id: document_id.into(),
            group: Some(DocumentGroup::from_pdf_type(&pdf_type)),
            source: DocumentGroupSource::Measured(DocumentGroupMeasured {
                pdf_type,
                classifier_id: classifier_id.into(),
                classifier_version: classifier_version.into(),
                pdf_sha256: pdf_sha256.into(),
            }),
        }
    }

    /// Build a typed-absence row.
    pub fn bytes_absent(document_id: impl Into<String>, reason: DocumentBytesAbsentReason) -> Self {
        Self {
            document_id: document_id.into(),
            group: None,
            source: DocumentGroupSource::BytesAbsent(DocumentGroupBytesAbsent { reason }),
        }
    }

    /// The assigned group, or `None` for a typed absence.
    pub fn assigned_group(&self) -> Option<DocumentGroup> {
        self.group
    }

    /// The typed reason this row is an absence, if it is one.
    pub fn bytes_absent_reason(&self) -> Option<DocumentBytesAbsentReason> {
        match &self.source {
            DocumentGroupSource::BytesAbsent(absent) => Some(absent.reason),
            DocumentGroupSource::Measured(_) => None,
        }
    }

    /// The required-family disposition this row's group forces, if any.
    pub fn forced_required_family_disposition(&self) -> Option<RequiredFamilyDisposition> {
        self.group
            .and_then(|group| group.forced_required_family_disposition())
    }

    /// Fail-closed self-consistency: a measured row must carry a group that
    /// matches its own `pdf_type`, and an absent row must not carry one.
    pub fn validate(&self) -> Result<(), DocumentGroupError> {
        if self.document_id.trim().is_empty() {
            return Err(DocumentGroupError::EmptyDocumentId);
        }
        match (&self.group, &self.source) {
            (Some(group), DocumentGroupSource::Measured(measured)) => {
                for (field, value) in [
                    ("classifier_id", &measured.classifier_id),
                    ("classifier_version", &measured.classifier_version),
                    ("pdf_sha256", &measured.pdf_sha256),
                ] {
                    if value.trim().is_empty() {
                        return Err(DocumentGroupError::EmptyMeasuredField {
                            document_id: self.document_id.clone(),
                            field,
                        });
                    }
                }
                let derived = DocumentGroup::from_pdf_type(&measured.pdf_type);
                if derived != *group {
                    return Err(DocumentGroupError::GroupPdfTypeMismatch {
                        document_id: self.document_id.clone(),
                        group: *group,
                        pdf_type: measured.pdf_type.clone(),
                    });
                }
            }
            (None, DocumentGroupSource::Measured(_)) => {
                return Err(DocumentGroupError::MeasuredWithoutGroup {
                    document_id: self.document_id.clone(),
                });
            }
            (Some(_), DocumentGroupSource::BytesAbsent(_)) => {
                return Err(DocumentGroupError::BytesAbsentCarriesGroup {
                    document_id: self.document_id.clone(),
                });
            }
            (None, DocumentGroupSource::BytesAbsent(_)) => {}
        }
        Ok(())
    }
}

// ───────────────────────── partition ─────────────────────────

/// Whether the declared corpus was actually partitioned, or was empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentGroupPartitionState {
    /// At least one document was declared, and every declared document has
    /// exactly one assignment.
    Partitioned,
    /// Nothing was declared. This is a typed absence, not a zero.
    EmptyDeclaredCorpus,
}

/// A total, disjoint partition of a declared corpus.
///
/// [`DocumentGroupPartition::assemble`] refuses anything that is not exactly one
/// assignment per declared document: a missing document, a duplicate, an
/// undeclared extra or an inconsistent row are all typed refusals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentGroupPartition {
    /// Schema version of this serialized partition.
    pub schema_version: String,
    /// Corpus identity the partition is bound to.
    pub corpus_id: String,
    /// How many documents were declared for this partition.
    pub declared_document_count: usize,
    /// Partitioned or typed-empty.
    pub state: DocumentGroupPartitionState,
    /// One assignment per declared document, in assembly order.
    pub assignments: Vec<DocumentGroupEvidence>,
}

impl DocumentGroupPartition {
    /// Assemble a partition that is total over `declared_document_ids`.
    ///
    /// An empty declaration yields [`DocumentGroupPartitionState::EmptyDeclaredCorpus`]
    /// and requires an empty assignment list.
    pub fn assemble(
        corpus_id: impl Into<String>,
        declared_document_ids: &[String],
        assignments: Vec<DocumentGroupEvidence>,
    ) -> Result<Self, DocumentGroupError> {
        let corpus_id = corpus_id.into();
        if corpus_id.trim().is_empty() {
            return Err(DocumentGroupError::EmptyCorpusId);
        }

        let mut declared = BTreeSet::new();
        for document_id in declared_document_ids {
            if !declared.insert(document_id.clone()) {
                return Err(DocumentGroupError::DuplicateDeclaredDocument {
                    document_id: document_id.clone(),
                });
            }
        }

        if declared.is_empty() {
            if !assignments.is_empty() {
                return Err(DocumentGroupError::AssignmentsForEmptyDeclaredCorpus {
                    count: assignments.len(),
                });
            }
            return Ok(Self {
                schema_version: DOCUMENT_GROUP_SCHEMA_VERSION.to_string(),
                corpus_id,
                declared_document_count: 0,
                state: DocumentGroupPartitionState::EmptyDeclaredCorpus,
                assignments,
            });
        }

        let mut seen = BTreeSet::new();
        for evidence in &assignments {
            evidence.validate()?;
            if !seen.insert(evidence.document_id.clone()) {
                return Err(DocumentGroupError::DuplicateDocumentAssignment {
                    document_id: evidence.document_id.clone(),
                });
            }
            if !declared.contains(&evidence.document_id) {
                return Err(DocumentGroupError::UndeclaredDocumentAssignment {
                    document_id: evidence.document_id.clone(),
                });
            }
        }
        for document_id in &declared {
            if !seen.contains(document_id) {
                return Err(DocumentGroupError::MissingDocumentAssignment {
                    document_id: document_id.clone(),
                });
            }
        }

        Ok(Self {
            schema_version: DOCUMENT_GROUP_SCHEMA_VERSION.to_string(),
            corpus_id,
            declared_document_count: declared.len(),
            state: DocumentGroupPartitionState::Partitioned,
            assignments,
        })
    }

    /// True when nothing was declared; the partition is a typed absence.
    pub fn is_empty_declared_corpus(&self) -> bool {
        self.state == DocumentGroupPartitionState::EmptyDeclaredCorpus
    }

    /// The assignment row for a declared document, if it exists.
    pub fn evidence_for(&self, document_id: &str) -> Option<&DocumentGroupEvidence> {
        self.assignments
            .iter()
            .find(|evidence| evidence.document_id == document_id)
    }

    /// The assigned group for a declared document. `None` means the document is
    /// not declared or its bytes were absent — never "assumed unknown".
    pub fn group_of(&self, document_id: &str) -> Option<DocumentGroup> {
        self.evidence_for(document_id)
            .and_then(DocumentGroupEvidence::assigned_group)
    }

    /// How many documents carry a measured group.
    pub fn measured_count(&self) -> usize {
        self.assignments
            .iter()
            .filter(|evidence| evidence.group.is_some())
            .count()
    }

    /// How many documents are a typed absence.
    pub fn bytes_absent_count(&self) -> usize {
        self.assignments
            .iter()
            .filter(|evidence| evidence.group.is_none())
            .count()
    }

    /// Group-stratified measured population. Absent documents are not counted
    /// in any group (they belong to no stratum).
    pub fn group_counts(&self) -> BTreeMap<DocumentGroup, usize> {
        let mut counts = BTreeMap::new();
        for evidence in &self.assignments {
            if let Some(group) = evidence.group {
                *counts.entry(group).or_insert(0) += 1;
            }
        }
        counts
    }

    /// The groups actually present, in canonical vocabulary order. Group
    /// strata are driven by this list, so a group with no member is not
    /// published as a zero-count stratum.
    pub fn groups_present(&self) -> Vec<DocumentGroup> {
        let counts = self.group_counts();
        [
            DocumentGroup::DigitalText,
            DocumentGroup::ScannedImage,
            DocumentGroup::Mixed,
            DocumentGroup::Unknown,
        ]
        .into_iter()
        .filter(|group| counts.contains_key(group))
        .collect()
    }

    /// The required-family disposition a document's group forces, if any.
    pub fn forced_required_family_disposition(
        &self,
        document_id: &str,
    ) -> Option<RequiredFamilyDisposition> {
        self.evidence_for(document_id)
            .and_then(DocumentGroupEvidence::forced_required_family_disposition)
    }
}

// ───────────────── required-family disposition ─────────────────

/// Closed reason a required family is `unmet`. The `out_of_scope` token is not
/// a member: `unsupported` is a decision about our own lane, and its only
/// honest representation is one of these four measured facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredFamilyUnmetReason {
    /// The document carries no text layer, so no text-bearing family can exist.
    NoTextLayer,
    /// The native lane structurally cannot produce this family.
    NativeLaneUnsupported,
    /// No independent oracle exists for this family.
    OracleAbsent,
    /// An oracle exists but carries no positive row, so it proves nothing.
    OracleVacuous,
}

impl RequiredFamilyUnmetReason {
    /// The serialized token for this arm.
    pub fn token(&self) -> &'static str {
        match self {
            RequiredFamilyUnmetReason::NoTextLayer => "no_text_layer",
            RequiredFamilyUnmetReason::NativeLaneUnsupported => "native_lane_unsupported",
            RequiredFamilyUnmetReason::OracleAbsent => "oracle_absent",
            RequiredFamilyUnmetReason::OracleVacuous => "oracle_vacuous",
        }
    }

    /// Fail-closed parse of the closed dictionary.
    pub fn from_token(token: &str) -> Result<Self, DocumentGroupError> {
        match token {
            "no_text_layer" => Ok(RequiredFamilyUnmetReason::NoTextLayer),
            "native_lane_unsupported" => Ok(RequiredFamilyUnmetReason::NativeLaneUnsupported),
            "oracle_absent" => Ok(RequiredFamilyUnmetReason::OracleAbsent),
            "oracle_vacuous" => Ok(RequiredFamilyUnmetReason::OracleVacuous),
            other => Err(DocumentGroupError::UnknownVocabularyToken {
                vocabulary: REQUIRED_FAMILY_UNMET_VOCABULARY_NAME,
                token: other.to_string(),
            }),
        }
    }
}

/// Closed disposition of a **required** family for one document.
///
/// There is no `out_of_scope` arm. A required family is either `measured`, or
/// `unmet` with a typed reason, or `not_observed` — and only a `measured`
/// disposition is a positive claim. Because the vocabulary has no
/// "out of scope", a downstream receipt cannot quietly reclassify a required
/// family as out of scope: the token is refused when it is parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredFamilyDisposition {
    /// The family was measured on this document.
    Measured,
    /// The family is required but could not be produced, for a typed reason.
    Unmet(RequiredFamilyUnmetReason),
    /// The family was not observed on this document. Not a claim of absence.
    NotObserved,
}

impl RequiredFamilyDisposition {
    /// Build an `unmet` disposition.
    pub fn unmet(reason: RequiredFamilyUnmetReason) -> Self {
        RequiredFamilyDisposition::Unmet(reason)
    }

    /// The unit token for this disposition (`unmet` for the reason-carrying
    /// arm; use [`RequiredFamilyDisposition::unmet_reason`] to read the reason).
    pub fn token(&self) -> &'static str {
        match self {
            RequiredFamilyDisposition::Measured => "measured",
            RequiredFamilyDisposition::Unmet(_) => "unmet",
            RequiredFamilyDisposition::NotObserved => "not_observed",
        }
    }

    /// True only for the positive arm.
    pub fn is_measured(&self) -> bool {
        matches!(self, RequiredFamilyDisposition::Measured)
    }

    /// The typed reason, if this is `unmet`.
    pub fn unmet_reason(&self) -> Option<RequiredFamilyUnmetReason> {
        match self {
            RequiredFamilyDisposition::Unmet(reason) => Some(*reason),
            RequiredFamilyDisposition::Measured | RequiredFamilyDisposition::NotObserved => None,
        }
    }

    /// Fail-closed parse of a unit disposition token.
    ///
    /// `out_of_scope` is refused **by name**
    /// ([`DocumentGroupError::RequiredFamilyOutOfScope`]) so a validator can
    /// report the exact mutation. `unmet` is refused without a reason, because
    /// an `unmet` with no reason is the same untyped hole this vocabulary
    /// exists to close.
    pub fn parse(token: &str) -> Result<Self, DocumentGroupError> {
        match token {
            "measured" => Ok(RequiredFamilyDisposition::Measured),
            "not_observed" => Ok(RequiredFamilyDisposition::NotObserved),
            "unmet" => Err(DocumentGroupError::UnmetRequiresReason),
            FORBIDDEN_REQUIRED_FAMILY_TOKEN => Err(DocumentGroupError::RequiredFamilyOutOfScope {
                token: token.to_string(),
            }),
            other => Err(DocumentGroupError::UnknownVocabularyToken {
                vocabulary: REQUIRED_FAMILY_DISPOSITION_VOCABULARY_NAME,
                token: other.to_string(),
            }),
        }
    }
}

// ───────────────────────────── errors ─────────────────────────────

/// Typed refusals from this module. Every variant names the document or the
/// vocabulary member that failed, so a caller never has to parse a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentGroupError {
    /// A corpus id was blank.
    EmptyCorpusId,
    /// A document id was blank.
    EmptyDocumentId,
    /// A measured row carried an empty provenance field.
    EmptyMeasuredField {
        /// Document the row belongs to.
        document_id: String,
        /// Which field was empty.
        field: &'static str,
    },
    /// The same id was declared twice.
    DuplicateDeclaredDocument {
        /// The repeated id.
        document_id: String,
    },
    /// The same id was assigned twice.
    DuplicateDocumentAssignment {
        /// The repeated id.
        document_id: String,
    },
    /// An assignment exists for a document that was never declared.
    UndeclaredDocumentAssignment {
        /// The undeclared id.
        document_id: String,
    },
    /// A declared document has no assignment.
    MissingDocumentAssignment {
        /// The unassigned id.
        document_id: String,
    },
    /// Assignments were supplied for an empty declaration.
    AssignmentsForEmptyDeclaredCorpus {
        /// How many assignments were supplied.
        count: usize,
    },
    /// A measured source carried no group.
    MeasuredWithoutGroup {
        /// Document the row belongs to.
        document_id: String,
    },
    /// A typed-absence row carried a group.
    BytesAbsentCarriesGroup {
        /// Document the row belongs to.
        document_id: String,
    },
    /// The stored group disagrees with the stored `pdf_type`.
    GroupPdfTypeMismatch {
        /// Document the row belongs to.
        document_id: String,
        /// The stored group.
        group: DocumentGroup,
        /// The raw classifier verdict.
        pdf_type: PdfType,
    },
    /// `unmet` was requested without a reason.
    UnmetRequiresReason,
    /// The forbidden required-family token was supplied.
    RequiredFamilyOutOfScope {
        /// The refused token.
        token: String,
    },
    /// A token outside a closed vocabulary was supplied.
    UnknownVocabularyToken {
        /// Which closed vocabulary was being parsed.
        vocabulary: &'static str,
        /// The refused token.
        token: String,
    },
}

impl fmt::Display for DocumentGroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentGroupError::EmptyCorpusId => write!(f, "corpus id must be non-empty"),
            DocumentGroupError::EmptyDocumentId => {
                write!(f, "document id must be non-empty")
            }
            DocumentGroupError::EmptyMeasuredField { document_id, field } => write!(
                f,
                "measured evidence for document `{document_id}` has an empty `{field}`"
            ),
            DocumentGroupError::DuplicateDeclaredDocument { document_id } => {
                write!(f, "document `{document_id}` was declared more than once")
            }
            DocumentGroupError::DuplicateDocumentAssignment { document_id } => {
                write!(f, "document `{document_id}` was assigned more than once")
            }
            DocumentGroupError::UndeclaredDocumentAssignment { document_id } => {
                write!(
                    f,
                    "document `{document_id}` was assigned but never declared"
                )
            }
            DocumentGroupError::MissingDocumentAssignment { document_id } => {
                write!(f, "declared document `{document_id}` has no assignment")
            }
            DocumentGroupError::AssignmentsForEmptyDeclaredCorpus { count } => write!(
                f,
                "{count} assignments were supplied for an empty declared corpus"
            ),
            DocumentGroupError::MeasuredWithoutGroup { document_id } => write!(
                f,
                "document `{document_id}` is measured but carries no group"
            ),
            DocumentGroupError::BytesAbsentCarriesGroup { document_id } => write!(
                f,
                "document `{document_id}` is a typed absence but carries a group"
            ),
            DocumentGroupError::GroupPdfTypeMismatch {
                document_id,
                group,
                pdf_type,
            } => write!(
                f,
                "document `{document_id}` stores group `{}` which does not match pdf_type `{pdf_type:?}`",
                group.token()
            ),
            DocumentGroupError::UnmetRequiresReason => {
                write!(f, "an `unmet` disposition requires a typed reason")
            }
            DocumentGroupError::RequiredFamilyOutOfScope { token } => write!(
                f,
                "required-family disposition refuses `{token}`: the required-family vocabulary has no out-of-scope arm"
            ),
            DocumentGroupError::UnknownVocabularyToken { vocabulary, token } => {
                write!(f, "`{token}` is not a member of `{vocabulary}`")
            }
        }
    }
}

impl std::error::Error for DocumentGroupError {}

// ───────────────────────────── tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn text_evidence(document_id: &str) -> DocumentGroupEvidence {
        DocumentGroupEvidence::measured(
            document_id,
            PdfType::Text,
            "pdf-inspector",
            "0.1.0",
            "sha256:aa",
        )
    }

    #[test]
    fn partition_is_total_and_disjoint() {
        let declared = vec!["hub".to_string(), "rlm".to_string(), "reca".to_string()];
        let partition = DocumentGroupPartition::assemble(
            "layout_bakeoff",
            &declared,
            vec![
                text_evidence("hub"),
                DocumentGroupEvidence::measured(
                    "rlm",
                    PdfType::Scanned,
                    "pdf-inspector",
                    "0.1.0",
                    "sha256:bb",
                ),
                DocumentGroupEvidence::measured(
                    "reca",
                    PdfType::Mixed,
                    "pdf-inspector",
                    "0.1.0",
                    "sha256:cc",
                ),
            ],
        )
        .expect("total partition");

        assert_eq!(partition.state, DocumentGroupPartitionState::Partitioned);
        assert_eq!(partition.declared_document_count, 3);
        assert_eq!(partition.measured_count(), 3);
        assert_eq!(partition.bytes_absent_count(), 0);
        assert_eq!(partition.group_of("hub"), Some(DocumentGroup::DigitalText));
        assert_eq!(partition.group_of("rlm"), Some(DocumentGroup::ScannedImage));
        assert_eq!(partition.group_of("reca"), Some(DocumentGroup::Mixed));
        assert_eq!(
            partition.groups_present(),
            vec![
                DocumentGroup::DigitalText,
                DocumentGroup::ScannedImage,
                DocumentGroup::Mixed,
            ]
        );
    }

    #[test]
    fn partition_refuses_gaps_overlaps_and_undeclared_extras() {
        let declared = vec!["hub".to_string(), "rlm".to_string()];

        let missing = DocumentGroupPartition::assemble("c", &declared, vec![text_evidence("hub")]);
        assert!(matches!(
            missing,
            Err(DocumentGroupError::MissingDocumentAssignment { document_id }) if document_id == "rlm"
        ));

        let duplicate = DocumentGroupPartition::assemble(
            "c",
            &declared,
            vec![
                text_evidence("hub"),
                text_evidence("hub"),
                text_evidence("rlm"),
            ],
        );
        assert!(matches!(
            duplicate,
            Err(DocumentGroupError::DuplicateDocumentAssignment { document_id }) if document_id == "hub"
        ));

        let undeclared = DocumentGroupPartition::assemble(
            "c",
            &declared,
            vec![
                text_evidence("hub"),
                text_evidence("rlm"),
                text_evidence("ghost"),
            ],
        );
        assert!(matches!(
            undeclared,
            Err(DocumentGroupError::UndeclaredDocumentAssignment { document_id }) if document_id == "ghost"
        ));

        let duplicated_declaration = DocumentGroupPartition::assemble(
            "c",
            &["hub".to_string(), "hub".to_string()],
            vec![text_evidence("hub")],
        );
        assert!(matches!(
            duplicated_declaration,
            Err(DocumentGroupError::DuplicateDeclaredDocument { document_id }) if document_id == "hub"
        ));
    }

    #[test]
    fn empty_declared_corpus_is_a_typed_absence_not_a_zero() {
        let partition =
            DocumentGroupPartition::assemble("c", &[], vec![]).expect("empty partition");
        assert!(partition.is_empty_declared_corpus());
        assert_eq!(
            partition.state,
            DocumentGroupPartitionState::EmptyDeclaredCorpus
        );
        assert_eq!(partition.declared_document_count, 0);
        assert_eq!(partition.measured_count(), 0);
        assert_eq!(partition.bytes_absent_count(), 0);
        assert!(partition.group_counts().is_empty());
        assert!(partition.groups_present().is_empty());
        assert_eq!(partition.group_of("hub"), None);

        let with_assignment =
            DocumentGroupPartition::assemble("c", &[], vec![text_evidence("hub")]);
        assert!(matches!(
            with_assignment,
            Err(DocumentGroupError::AssignmentsForEmptyDeclaredCorpus { count: 1 })
        ));
    }

    #[test]
    fn bytes_absent_is_never_folded_into_unknown() {
        let declared = vec!["hub".to_string(), "rlm".to_string()];
        let partition = DocumentGroupPartition::assemble(
            "c",
            &declared,
            vec![
                text_evidence("hub"),
                DocumentGroupEvidence::bytes_absent(
                    "rlm",
                    DocumentBytesAbsentReason::PdfBytesMissing,
                ),
            ],
        )
        .expect("partition with absence");

        assert_eq!(partition.group_of("rlm"), None);
        assert_eq!(partition.bytes_absent_count(), 1);
        assert_eq!(partition.measured_count(), 1);
        assert!(
            !partition
                .group_counts()
                .contains_key(&DocumentGroup::Unknown)
        );
        assert_eq!(partition.groups_present(), vec![DocumentGroup::DigitalText]);
        assert_eq!(
            partition
                .evidence_for("rlm")
                .and_then(DocumentGroupEvidence::bytes_absent_reason),
            Some(DocumentBytesAbsentReason::PdfBytesMissing)
        );
        assert_eq!(partition.forced_required_family_disposition("rlm"), None);

        // A typed absence that carries a group is refused, and a measured row
        // without one is refused: the two facts cannot be conflated.
        let smuggled = DocumentGroupEvidence {
            document_id: "rlm".into(),
            group: Some(DocumentGroup::Unknown),
            source: DocumentGroupSource::BytesAbsent(DocumentGroupBytesAbsent {
                reason: DocumentBytesAbsentReason::PdfReadFailed,
            }),
        };
        assert!(matches!(
            smuggled.validate(),
            Err(DocumentGroupError::BytesAbsentCarriesGroup { .. })
        ));

        let headless = DocumentGroupEvidence {
            document_id: "rlm".into(),
            group: None,
            source: DocumentGroupSource::Measured(DocumentGroupMeasured {
                pdf_type: PdfType::Text,
                classifier_id: "pdf-inspector".into(),
                classifier_version: "0.1.0".into(),
                pdf_sha256: "sha256:bb".into(),
            }),
        };
        assert!(matches!(
            headless.validate(),
            Err(DocumentGroupError::MeasuredWithoutGroup { .. })
        ));
    }

    #[test]
    fn measured_group_must_match_the_mapped_pdf_type() {
        let inconsistent = DocumentGroupEvidence {
            document_id: "hub".into(),
            group: Some(DocumentGroup::DigitalText),
            source: DocumentGroupSource::Measured(DocumentGroupMeasured {
                pdf_type: PdfType::Scanned,
                classifier_id: "pdf-inspector".into(),
                classifier_version: "0.1.0".into(),
                pdf_sha256: "sha256:aa".into(),
            }),
        };
        assert!(matches!(
            inconsistent.validate(),
            Err(DocumentGroupError::GroupPdfTypeMismatch {
                group: DocumentGroup::DigitalText,
                ..
            })
        ));

        let empty_field =
            DocumentGroupEvidence::measured("hub", PdfType::Text, "", "0.1.0", "sha256:aa");
        assert!(matches!(
            empty_field.validate(),
            Err(DocumentGroupError::EmptyMeasuredField {
                field: "classifier_id",
                ..
            })
        ));
    }

    #[test]
    fn scanned_group_forces_unmet_no_text_layer() {
        assert_eq!(
            DocumentGroup::ScannedImage.text_layer_absence(),
            Some(RequiredFamilyUnmetReason::NoTextLayer)
        );
        assert_eq!(
            DocumentGroup::ScannedImage.forced_required_family_disposition(),
            Some(RequiredFamilyDisposition::Unmet(
                RequiredFamilyUnmetReason::NoTextLayer
            ))
        );
        for group in [
            DocumentGroup::DigitalText,
            DocumentGroup::Mixed,
            DocumentGroup::Unknown,
        ] {
            assert_eq!(group.text_layer_absence(), None, "{group:?}");
            assert_eq!(
                group.forced_required_family_disposition(),
                None,
                "{group:?}"
            );
        }

        let partition = DocumentGroupPartition::assemble(
            "c",
            &["rlm".to_string()],
            vec![DocumentGroupEvidence::measured(
                "rlm",
                PdfType::Scanned,
                "pdf-inspector",
                "0.1.0",
                "sha256:bb",
            )],
        )
        .expect("partition");
        assert_eq!(
            partition.forced_required_family_disposition("rlm"),
            Some(RequiredFamilyDisposition::Unmet(
                RequiredFamilyUnmetReason::NoTextLayer
            ))
        );
    }

    #[test]
    fn partition_roundtrips_through_json() {
        let partition = DocumentGroupPartition::assemble(
            "c",
            &["hub".to_string(), "rlm".to_string()],
            vec![
                text_evidence("hub"),
                DocumentGroupEvidence::bytes_absent(
                    "rlm",
                    DocumentBytesAbsentReason::ClassifierUnavailable,
                ),
            ],
        )
        .expect("partition");
        let json = serde_json::to_string(&partition).expect("serialize");
        let back: DocumentGroupPartition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(partition, back);

        let measured = serde_json::to_value(text_evidence("hub")).expect("value");
        assert_eq!(measured["group"], "digital_text");
        assert_eq!(measured["source"]["kind"], "measured");
        assert_eq!(measured["source"]["pdf_type"], "text");

        let absent = serde_json::to_value(DocumentGroupEvidence::bytes_absent(
            "rlm",
            DocumentBytesAbsentReason::PdfBytesMissing,
        ))
        .expect("value");
        assert!(absent.get("group").is_none(), "absence carries no group");
        assert_eq!(absent["source"]["kind"], "bytes_absent");
        assert_eq!(absent["source"]["reason"], "pdf_bytes_missing");
    }
}
