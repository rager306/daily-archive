//! Revision-grounded equation candidate domain contract (M400 S03, ADR-051).
//!
//! An [`EquationCandidate`] is deterministic, non-authoritative graph evidence
//! derived from a TEI `<formula>` element. Identity binds the PaperRevision,
//! formula id, exact revision-scoped Section VID, and formula source digest.
//! The representation and its source span are preserved verbatim; no MathML,
//! LaTeX, or plain-text representation is invented. Authority and import are
//! hard-locked false (D127 / ADR-053).

use crate::vid::sha256_vid;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current EquationCandidate DATA schema version.
pub const EQUATION_CANDIDATE_SCHEMA_VERSION: u32 = 1;

/// Canonical representation kind wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquationRepresentationKind {
    MathMl,
    TexMath,
    PlainText,
}

impl EquationRepresentationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MathMl => "math_ml",
            Self::TexMath => "tex_math",
            Self::PlainText => "plain_text",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value.trim() {
            "math_ml" => Some(Self::MathMl),
            "tex_math" => Some(Self::TexMath),
            "plain_text" => Some(Self::PlainText),
            _ => None,
        }
    }
}

/// Deterministic revision-bound EquationCandidate VID.
///
/// Binds revision identity, TEI formula id, exact revision-scoped Section VID,
/// and formula source digest. Repeating one revision is deterministic; two
/// revisions produce disjoint equation candidates.
pub fn equation_candidate_vid(
    revision_vid: &str,
    formula_id: &str,
    section_vid: &str,
    source_digest: &str,
) -> Result<String, String> {
    let revision = normalize_sha256(revision_vid, "revision VID")?;
    let formula = formula_id.trim();
    if formula.is_empty() {
        return Err("formula_id must not be empty".into());
    }
    let section = normalize_sha256(section_vid, "section VID")?;
    let source = normalize_sha256(source_digest, "formula source digest")?;
    Ok(sha256_vid(
        "equation_candidate",
        &format!("{revision}\n{formula}\n{section}\n{source}"),
    ))
}

fn normalize_sha256(value: &str, field: &'static str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a 64-character SHA-256 hex digest"));
    }
    Ok(normalized)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Pure input used to assemble a domain EquationCandidate. The application
/// mapper converts `da_ports::parser::ExtractedFormula` into this shape; the
/// domain remains independent of the ports crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquationCandidateInput {
    pub revision_vid: String,
    pub source_pdf_sha256: String,
    pub formula_id: String,
    pub section_vid: String,
    pub section_path_id: String,
    pub section_ordinal: u32,
    pub section_title: String,
    pub label: Option<String>,
    pub coordinates: Option<String>,
    pub representation_kind: EquationRepresentationKind,
    pub representation_text: String,
    pub span_start: Option<u32>,
    pub span_end: Option<u32>,
    pub source_digest: String,
}

/// Non-authoritative revision-grounded equation candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquationCandidate {
    pub vid: String,
    pub revision_vid: String,
    pub source_pdf_sha256: String,
    pub formula_id: String,
    pub section_vid: String,
    pub section_path_id: String,
    pub section_ordinal: u32,
    pub section_title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<String>,
    pub representation_kind: EquationRepresentationKind,
    pub representation_text: String,
    pub representation_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_end: Option<u32>,
    pub source_digest: String,
    pub candidate_metadata_sha256: String,
    pub authoritative: bool,
    pub import_eligible: bool,
    pub schema_version: u32,
}

impl EquationCandidate {
    pub fn new(input: EquationCandidateInput) -> Result<Self, EquationCandidateError> {
        let vid = equation_candidate_vid(
            &input.revision_vid,
            &input.formula_id,
            &input.section_vid,
            &input.source_digest,
        )
        .map_err(EquationCandidateError::Invalid)?;
        let representation_sha256 = sha256_vid(
            "equation_representation",
            &format!(
                "{}\n{}",
                input.representation_kind.as_str(),
                input.representation_text
            ),
        );
        let candidate_metadata_sha256 = sha256_vid(
            "equation_candidate_metadata",
            &format!(
                "{vid}\n{}\n{}\n{}\n{}\n{}\n{:?}\n{:?}\n{}\n{}",
                input.source_pdf_sha256,
                input.section_path_id,
                input.section_ordinal,
                input.section_title,
                input.label.as_deref().unwrap_or(""),
                input.coordinates,
                (input.span_start, input.span_end),
                representation_sha256,
                input.source_digest,
            ),
        );
        let candidate = Self {
            vid,
            revision_vid: input.revision_vid,
            source_pdf_sha256: input.source_pdf_sha256,
            formula_id: input.formula_id,
            section_vid: input.section_vid,
            section_path_id: input.section_path_id,
            section_ordinal: input.section_ordinal,
            section_title: input.section_title,
            label: input.label,
            coordinates: input.coordinates,
            representation_kind: input.representation_kind,
            representation_text: input.representation_text,
            representation_sha256,
            span_start: input.span_start,
            span_end: input.span_end,
            source_digest: input.source_digest,
            candidate_metadata_sha256,
            authoritative: false,
            import_eligible: false,
            schema_version: EQUATION_CANDIDATE_SCHEMA_VERSION,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn recompute_vid(&self) -> Result<String, String> {
        equation_candidate_vid(
            &self.revision_vid,
            &self.formula_id,
            &self.section_vid,
            &self.source_digest,
        )
    }

    pub fn identity_is_consistent(&self) -> bool {
        self.recompute_vid().is_ok_and(|vid| vid == self.vid)
    }

    pub fn validate(&self) -> Result<(), EquationCandidateError> {
        for (field, value) in [
            ("vid", self.vid.as_str()),
            ("revision_vid", self.revision_vid.as_str()),
            ("section_vid", self.section_vid.as_str()),
            ("source_pdf_sha256", self.source_pdf_sha256.as_str()),
            ("representation_sha256", self.representation_sha256.as_str()),
            ("source_digest", self.source_digest.as_str()),
            (
                "candidate_metadata_sha256",
                self.candidate_metadata_sha256.as_str(),
            ),
        ] {
            if !is_sha256(value) {
                return Err(EquationCandidateError::InvalidSha256(field));
            }
        }
        for (field, value) in [
            ("formula_id", self.formula_id.as_str()),
            ("section_path_id", self.section_path_id.as_str()),
            ("section_title", self.section_title.as_str()),
            ("representation_text", self.representation_text.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(EquationCandidateError::Missing(field));
            }
        }
        if let (Some(start), Some(end)) = (self.span_start, self.span_end) {
            if end < start {
                return Err(EquationCandidateError::InvalidSpan { start, end });
            }
        } else if self.span_start.is_some() || self.span_end.is_some() {
            return Err(EquationCandidateError::PartialSpan);
        }
        if self.schema_version == 0 {
            return Err(EquationCandidateError::Invalid(
                "schema_version must be greater than zero".into(),
            ));
        }
        if self.authoritative || self.import_eligible {
            return Err(EquationCandidateError::AuthorityViolation);
        }
        if !self.identity_is_consistent() {
            return Err(EquationCandidateError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EquationCandidateError {
    #[error("missing equation candidate field: {0}")]
    Missing(&'static str),
    #[error("{0} must be a 64-character SHA-256 hex digest")]
    InvalidSha256(&'static str),
    #[error("invalid equation source span: start {start} is greater than end {end}")]
    InvalidSpan { start: u32, end: u32 },
    #[error("equation source span must carry both start and end or neither")]
    PartialSpan,
    #[error("invalid equation candidate: {0}")]
    Invalid(String),
    #[error("equation candidate cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("equation candidate VID does not match revision-bound lineage")]
    IdentityMismatch,
}

/// Schema-as-code registration mirroring `data/ontology/node_schemas.yaml`.
#[derive(Debug, Default)]
pub struct EquationCandidateSchema;

impl crate::schema::NodeSchemaDef for EquationCandidateSchema {
    fn label(&self) -> &'static str {
        "EquationCandidate"
    }

    fn required_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        use crate::schema::FieldType;
        vec![
            ("vid", FieldType::String),
            ("revision_vid", FieldType::String),
            ("source_pdf_sha256", FieldType::String),
            ("formula_id", FieldType::String),
            ("section_vid", FieldType::String),
            ("section_path_id", FieldType::String),
            ("section_ordinal", FieldType::Integer),
            ("section_title", FieldType::String),
            ("representation_kind", FieldType::String),
            ("representation_text", FieldType::String),
            ("representation_sha256", FieldType::String),
            ("source_digest", FieldType::String),
            ("candidate_metadata_sha256", FieldType::String),
            ("authoritative", FieldType::Boolean),
            ("import_eligible", FieldType::Boolean),
            ("schema_version", FieldType::Integer),
        ]
    }

    fn optional_fields(&self) -> Vec<(&'static str, crate::schema::FieldType)> {
        use crate::schema::FieldType;
        vec![
            ("label", FieldType::String),
            ("coordinates", FieldType::String),
            ("span_start", FieldType::Integer),
            ("span_end", FieldType::Integer),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

    const REVISION_A: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn input(revision_vid: &str) -> EquationCandidateInput {
        EquationCandidateInput {
            revision_vid: revision_vid.into(),
            source_pdf_sha256: "a".repeat(64),
            formula_id: "formula_0".into(),
            section_vid: "b".repeat(64),
            section_path_id: "3.2".into(),
            section_ordinal: 7,
            section_title: "Experimental Setup".into(),
            label: Some("(1)".into()),
            coordinates: Some("10,20,30,40;5".into()),
            representation_kind: EquationRepresentationKind::PlainText,
            representation_text: "L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]".into(),
            span_start: Some(120),
            span_end: Some(158),
            source_digest: "c".repeat(64),
        }
    }

    fn candidate(revision_vid: &str) -> EquationCandidate {
        EquationCandidate::new(input(revision_vid)).unwrap()
    }

    #[test]
    fn candidate_roundtrips_with_revision_section_lineage() {
        let value = candidate(REVISION_A);
        value.validate().unwrap();
        crate::roundtrip_json(&value);
        let decoded: EquationCandidate =
            serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.representation_kind.as_str(), "plain_text");
        assert!(!decoded.authoritative && !decoded.import_eligible);
    }

    #[test]
    fn rejects_authority_or_import_unlock() {
        let mut value = candidate(REVISION_A);
        value.authoritative = true;
        assert_eq!(
            value.validate(),
            Err(EquationCandidateError::AuthorityViolation)
        );
        let mut value = candidate(REVISION_A);
        value.import_eligible = !value.import_eligible;
        assert_eq!(
            value.validate(),
            Err(EquationCandidateError::AuthorityViolation)
        );
    }

    #[test]
    fn rejects_invalid_and_partial_spans() {
        let mut value = candidate(REVISION_A);
        value.span_start = Some(10);
        value.span_end = Some(9);
        assert_eq!(
            value.validate(),
            Err(EquationCandidateError::InvalidSpan { start: 10, end: 9 })
        );
        let mut value = candidate(REVISION_A);
        value.span_end = None;
        assert_eq!(value.validate(), Err(EquationCandidateError::PartialSpan));
    }

    #[test]
    fn rejects_malformed_sha256_lineage() {
        let mut value = candidate(REVISION_A);
        value.source_digest = "not-a-digest".into();
        assert_eq!(
            value.validate(),
            Err(EquationCandidateError::InvalidSha256("source_digest"))
        );
    }

    #[test]
    fn rejects_tampered_vid() {
        let mut value = candidate(REVISION_A);
        value.vid = "f".repeat(64);
        assert_eq!(
            value.validate(),
            Err(EquationCandidateError::IdentityMismatch)
        );
    }

    fn hex64() -> impl Strategy<Value = String> {
        vec(prop_oneof![Just('a'), Just('b'), Just('0'), Just('9')], 64)
            .prop_map(|chars| chars.into_iter().collect())
    }

    proptest! {
        #[test]
        fn prop_candidate_identity_is_deterministic_and_revision_disjoint(
            revision in hex64(),
            formula in "(formula_|eq_)[A-Za-z0-9_]{1,12}",
            section in hex64(),
            digest in hex64(),
        ) {
            let a = equation_candidate_vid(&revision, &formula, &section, &digest).unwrap();
            let b = equation_candidate_vid(&revision, &formula, &section, &digest).unwrap();
            prop_assert_eq!(&a, &b);
            let mut other = revision.clone();
            if other.as_bytes()[0] == b'a' {
                other.replace_range(0..1, "b");
            } else {
                other.replace_range(0..1, "a");
            }
            let c = equation_candidate_vid(&other, &formula, &section, &digest).unwrap();
            prop_assert_ne!(&a, &c);
        }

        #[test]
        fn prop_candidate_identity_binds_every_component(
            revision in hex64(),
            section in hex64(),
            digest in hex64(),
        ) {
            let base = equation_candidate_vid(&revision, "formula_0", &section, &digest).unwrap();
            let formula = equation_candidate_vid(&revision, "formula_1", &section, &digest).unwrap();
            let mut section_other = section.clone();
            section_other.replace_range(0..1, if section.starts_with('a') { "b" } else { "a" });
            let section_changed = equation_candidate_vid(&revision, "formula_0", &section_other, &digest).unwrap();
            let mut digest_other = digest.clone();
            digest_other.replace_range(0..1, if digest.starts_with('a') { "b" } else { "a" });
            let digest_changed = equation_candidate_vid(&revision, "formula_0", &section, &digest_other).unwrap();
            prop_assert_ne!(&base, &formula);
            prop_assert_ne!(&base, &section_changed);
            prop_assert_ne!(&base, &digest_changed);
        }

        #[test]
        fn prop_valid_input_always_validates(revision in hex64(), section in hex64(), digest in hex64()) {
            let mut value = input(&revision);
            value.section_vid = section;
            value.source_digest = digest;
            let candidate = EquationCandidate::new(value).unwrap();
            prop_assert!(candidate.validate().is_ok());
            prop_assert!(!candidate.authoritative);
            prop_assert!(!candidate.import_eligible);
        }
    }
}
