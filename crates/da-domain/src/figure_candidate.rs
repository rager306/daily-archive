//! Revision-grounded figure candidate domain contract (M400 S01, ADR-051).
//!
//! A `FigureCandidate` is deterministic, revision-bound, non-authoritative
//! graph evidence materialized from the detector and analysis planes. Its
//! identity is a SHA-256 VID over the revision, detector artifact label,
//! detector fingerprint, rendered image content hash, and the exact
//! revision-scoped section VID. Every SHA-256 lineage field and the section
//! identity are validated; authority and import unlock are rejected.
//! Candidate evidence is never graph truth (D127 / ADR-053).

use crate::VisualArtifactKind;
use crate::vid::sha256_vid;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current FigureCandidate DATA schema version.
pub const FIGURE_CANDIDATE_SCHEMA_VERSION: u32 = 1;

/// Deterministic revision-bound candidate VID.
///
/// Binds revision identity, detector artifact label, detector fingerprint,
/// rendered image content hash, and the exact revision-scoped Section VID.
/// Two revisions of the same artifact produce disjoint candidates; an
/// identical lineage replays onto the same candidate identity.
pub fn figure_candidate_vid(
    revision_vid: &str,
    artifact_id: &str,
    detector_fingerprint: &str,
    image_sha256: &str,
    section_vid: &str,
) -> Result<String, String> {
    let revision = normalize_sha256(revision_vid, "revision VID")?;
    let artifact = artifact_id.trim();
    if artifact.is_empty() {
        return Err("artifact_id must not be empty".into());
    }
    let detector = detector_fingerprint.trim();
    if detector.is_empty() {
        return Err("detector_fingerprint must not be empty".into());
    }
    let image = normalize_sha256(image_sha256, "image sha256")?;
    let section = normalize_sha256(section_vid, "section VID")?;
    Ok(sha256_vid(
        "figure_candidate",
        &format!("{revision}\n{artifact}\n{detector}\n{image}\n{section}"),
    ))
}

/// Normalize a SHA-256 lineage value to canonical lowercase hex form.
fn normalize_sha256(value: &str, field: &'static str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a 64-character SHA-256 hex digest"));
    }
    Ok(normalized)
}

/// Non-authoritative revision-grounded figure candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FigureCandidate {
    /// Deterministic revision-bound candidate VID ([`figure_candidate_vid`]).
    pub vid: String,
    /// PaperRevision VID this candidate is grounded in.
    pub revision_vid: String,
    /// Detector artifact label, e.g. `Figure3`.
    pub artifact_id: String,
    pub kind: VisualArtifactKind,
    pub detector_name: String,
    pub detector_fingerprint: String,
    /// SHA-256 of the detector metadata record that produced this artifact.
    pub detector_metadata_sha256: String,
    /// SHA-256 of the source PDF manifestation this revision is bound to.
    pub source_pdf_sha256: String,
    /// SHA-256 of the rendered figure asset at `asset_path`.
    pub image_sha256: String,
    /// Canonical asset path relative to the article directory (figures/…).
    pub asset_path: String,
    /// Zero-based page index (pdffigures2 semantics).
    pub page_index: u32,
    /// Revision-scoped Section VID (`revision_section_vid`) — exact lineage.
    pub section_vid: String,
    pub section_title: String,
    pub section_ordinal: u32,
    pub caption: String,
    /// SHA-256 of the candidate record itself (deterministic replay evidence).
    pub candidate_metadata_sha256: String,
    /// Authority lock — must remain `false` (candidate evidence, D127).
    pub authoritative: bool,
    /// Import lock — must remain `false`.
    pub import_eligible: bool,
    pub schema_version: u32,
    /// Deterministic materialization diagnostics (replay outcome), written by
    /// the materializer on write; absent until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialization_diagnostics: Option<String>,
}

impl FigureCandidate {
    /// Recompute the candidate VID from its revision-bound lineage fields.
    pub fn recompute_vid(&self) -> Result<String, String> {
        figure_candidate_vid(
            &self.revision_vid,
            &self.artifact_id,
            &self.detector_fingerprint,
            &self.image_sha256,
            &self.section_vid,
        )
    }

    /// True when the stored VID matches the recomputed revision-bound lineage.
    pub fn identity_is_consistent(&self) -> bool {
        self.recompute_vid().is_ok_and(|vid| vid == self.vid)
    }

    /// Validate every SHA-256 lineage field, section identity, authority
    /// locks, and deterministic identity.
    pub fn validate(&self) -> Result<(), FigureCandidateError> {
        for (field, value) in [
            ("vid", self.vid.as_str()),
            ("revision_vid", self.revision_vid.as_str()),
            ("section_vid", self.section_vid.as_str()),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(FigureCandidateError::InvalidSha256(field));
            }
        }
        for (field, value) in [
            ("source_pdf_sha256", self.source_pdf_sha256.as_str()),
            (
                "detector_metadata_sha256",
                self.detector_metadata_sha256.as_str(),
            ),
            ("image_sha256", self.image_sha256.as_str()),
            (
                "candidate_metadata_sha256",
                self.candidate_metadata_sha256.as_str(),
            ),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(FigureCandidateError::InvalidSha256(field));
            }
        }
        for (field, value) in [
            ("artifact_id", self.artifact_id.as_str()),
            ("detector_name", self.detector_name.as_str()),
            ("detector_fingerprint", self.detector_fingerprint.as_str()),
            ("section_title", self.section_title.as_str()),
            ("caption", self.caption.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(FigureCandidateError::Missing(field));
            }
        }
        if !self.asset_path.starts_with("figures/")
            || self.asset_path.contains("..")
            || self.asset_path.starts_with('/')
        {
            return Err(FigureCandidateError::InvalidAssetPath(
                self.asset_path.clone(),
            ));
        }
        if self.schema_version == 0 {
            return Err(FigureCandidateError::Invalid(
                "schema_version must be greater than zero".into(),
            ));
        }
        if self.authoritative || self.import_eligible {
            return Err(FigureCandidateError::AuthorityViolation);
        }
        if !self.identity_is_consistent() {
            return Err(FigureCandidateError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FigureCandidateError {
    #[error("missing figure candidate field: {0}")]
    Missing(&'static str),
    #[error("{0} must be a 64-character SHA-256 hex digest")]
    InvalidSha256(&'static str),
    #[error("invalid canonical figure asset path: {0}")]
    InvalidAssetPath(String),
    #[error("invalid figure candidate: {0}")]
    Invalid(String),
    #[error("figure candidate cannot be authoritative or import eligible")]
    AuthorityViolation,
    #[error("figure candidate VID does not match revision-bound lineage")]
    IdentityMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vid::revision_section_vid;

    const REVISION_A: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const REVISION_B: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn candidate(revision_vid: &str) -> FigureCandidate {
        let section_vid = revision_section_vid(revision_vid, 2).unwrap();
        let vid = figure_candidate_vid(
            revision_vid,
            "Figure3",
            "pdffigures2-0.0.12-SNAPSHOT",
            &"b".repeat(64),
            &section_vid,
        )
        .unwrap();
        FigureCandidate {
            vid,
            revision_vid: revision_vid.into(),
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "pdffigures2-0.0.12-SNAPSHOT".into(),
            detector_metadata_sha256: "c".repeat(64),
            source_pdf_sha256: "a".repeat(64),
            image_sha256: "b".repeat(64),
            asset_path: "figures/Figure3.png".into(),
            page_index: 3,
            section_vid,
            section_title: "GEPA System".into(),
            section_ordinal: 2,
            caption: "Figure 3: Overview of the GEPA system.".into(),
            candidate_metadata_sha256: "d".repeat(64),
            authoritative: false,
            import_eligible: false,
            schema_version: FIGURE_CANDIDATE_SCHEMA_VERSION,
            materialization_diagnostics: None,
        }
    }

    #[test]
    fn candidate_roundtrips_with_revision_section_lineage() {
        let value = candidate(REVISION_A);
        value.validate().unwrap();
        crate::roundtrip_json(&value);
        let json = serde_json::to_string(&value).unwrap();
        let decoded: FigureCandidate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.section_ordinal, 2);
        assert_eq!(decoded.revision_vid, REVISION_A);
        assert!(!decoded.authoritative && !decoded.import_eligible);
    }

    #[test]
    fn rejects_authority_or_import_unlock() {
        let mut value = candidate(REVISION_A);
        value.authoritative = true;
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::AuthorityViolation)
        );
        let mut value = candidate(REVISION_A);
        value.import_eligible = !value.import_eligible;
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::AuthorityViolation)
        );
    }

    #[test]
    fn rejects_malformed_sha256_lineage_fields() {
        for field in [
            "source_pdf_sha256",
            "detector_metadata_sha256",
            "image_sha256",
            "candidate_metadata_sha256",
        ] {
            let mut value = candidate(REVISION_A);
            match field {
                "source_pdf_sha256" => value.source_pdf_sha256 = "not-sha".into(),
                "detector_metadata_sha256" => value.detector_metadata_sha256 = "not-sha".into(),
                "image_sha256" => value.image_sha256 = "not-sha".into(),
                "candidate_metadata_sha256" => value.candidate_metadata_sha256 = "not-sha".into(),
                _ => unreachable!(),
            }
            assert_eq!(
                value.validate(),
                Err(FigureCandidateError::InvalidSha256(field)),
                "{field} must be validated"
            );
        }
        // VIDs themselves are lineage fields too.
        let mut value = candidate(REVISION_A);
        value.revision_vid = "short".into();
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::InvalidSha256("revision_vid"))
        );
    }

    #[test]
    fn rejects_missing_section_identity() {
        let mut value = candidate(REVISION_A);
        value.section_vid = "".into();
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::InvalidSha256("section_vid"))
        );
        let mut value = candidate(REVISION_A);
        value.section_title = "   ".into();
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::Missing("section_title"))
        );
    }

    #[test]
    fn rejects_tampered_candidate_vid() {
        let mut value = candidate(REVISION_A);
        value.vid = "f".repeat(64);
        assert_eq!(
            value.validate(),
            Err(FigureCandidateError::IdentityMismatch)
        );
    }

    #[test]
    fn rejects_traversal_or_non_canonical_asset_paths() {
        for path in ["/tmp/Figure3.png", "figures/../source.pdf", "Figure3.png"] {
            let mut value = candidate(REVISION_A);
            value.asset_path = path.into();
            assert!(matches!(
                value.validate(),
                Err(FigureCandidateError::InvalidAssetPath(_))
            ));
        }
    }

    #[test]
    fn candidate_vids_are_revision_bound_and_disjoint() {
        let a = figure_candidate_vid(
            REVISION_A,
            "Figure3",
            "pdffigures2-0.0.12-SNAPSHOT",
            &"b".repeat(64),
            &revision_section_vid(REVISION_A, 2).unwrap(),
        )
        .unwrap();
        let a_again = figure_candidate_vid(
            REVISION_A,
            "Figure3",
            "pdffigures2-0.0.12-SNAPSHOT",
            &"b".repeat(64),
            &revision_section_vid(REVISION_A, 2).unwrap(),
        )
        .unwrap();
        let b = figure_candidate_vid(
            REVISION_B,
            "Figure3",
            "pdffigures2-0.0.12-SNAPSHOT",
            &"b".repeat(64),
            &revision_section_vid(REVISION_B, 2).unwrap(),
        )
        .unwrap();
        assert_eq!(a, a_again, "identical lineage replays to the same VID");
        assert_ne!(a, b, "two revisions must produce disjoint candidates");
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn candidate_vids_distinguish_sections_within_a_revision() {
        let section_2 = revision_section_vid(REVISION_A, 2).unwrap();
        let section_3 = revision_section_vid(REVISION_A, 3).unwrap();
        let v2 = figure_candidate_vid(
            REVISION_A,
            "Figure3",
            "pdffigures2",
            &"b".repeat(64),
            &section_2,
        )
        .unwrap();
        let v3 = figure_candidate_vid(
            REVISION_A,
            "Figure3",
            "pdffigures2",
            &"b".repeat(64),
            &section_3,
        )
        .unwrap();
        assert_ne!(v2, v3, "section lineage is part of candidate identity");
    }

    #[test]
    fn candidate_vid_rejects_malformed_identity_inputs() {
        assert!(
            figure_candidate_vid(
                "not-hex",
                "Figure3",
                "pdffigures2",
                &"b".repeat(64),
                &"c".repeat(64)
            )
            .is_err()
        );
        assert!(
            figure_candidate_vid(
                &"a".repeat(64),
                "",
                "pdffigures2",
                &"b".repeat(64),
                &"c".repeat(64)
            )
            .is_err()
        );
        assert!(
            figure_candidate_vid(
                &"a".repeat(64),
                "Figure3",
                "",
                &"b".repeat(64),
                &"c".repeat(64)
            )
            .is_err()
        );
        assert!(
            figure_candidate_vid(
                &"a".repeat(64),
                "Figure3",
                "pdffigures2",
                "short",
                &"c".repeat(64)
            )
            .is_err()
        );
        assert!(
            figure_candidate_vid(
                &"a".repeat(64),
                "Figure3",
                "pdffigures2",
                &"b".repeat(64),
                "not-hex"
            )
            .is_err()
        );
    }

    #[test]
    fn schema_version_zero_is_rejected() {
        let mut value = candidate(REVISION_A);
        value.schema_version = 0;
        assert!(value.validate().is_err());
    }
}

// ─── ADR-051 P1: Property-based tests (M400 S01) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    fn sha256_hex() -> impl Strategy<Value = String> {
        "[0-9a-f]{64}".prop_map(|s| s)
    }

    fn nonempty() -> impl Strategy<Value = String> {
        "[A-Za-z0-9._-]{1,32}".prop_map(|s| s)
    }

    fn valid_candidate(
        revision_vid: &str,
        artifact_id: &str,
        detector_fingerprint: &str,
        image_sha256: &str,
        section_vid: &str,
    ) -> FigureCandidate {
        let vid = figure_candidate_vid(
            revision_vid,
            artifact_id,
            detector_fingerprint,
            image_sha256,
            section_vid,
        )
        .unwrap();
        FigureCandidate {
            vid,
            revision_vid: revision_vid.into(),
            artifact_id: artifact_id.into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: detector_fingerprint.into(),
            detector_metadata_sha256: "c".repeat(64),
            source_pdf_sha256: "a".repeat(64),
            image_sha256: image_sha256.into(),
            asset_path: "figures/Figure3.png".into(),
            page_index: 3,
            section_vid: section_vid.into(),
            section_title: "GEPA System".into(),
            section_ordinal: 2,
            caption: "Figure 3: Overview of the GEPA system.".into(),
            candidate_metadata_sha256: "d".repeat(64),
            authoritative: false,
            import_eligible: false,
            schema_version: FIGURE_CANDIDATE_SCHEMA_VERSION,
            materialization_diagnostics: None,
        }
    }

    proptest! {
        /// Candidate VID is deterministic and revisions are disjoint.
        #[test]
        fn prop_candidate_vid_is_deterministic_and_revision_disjoint(
            rev_a in sha256_hex(),
            rev_b in sha256_hex(),
            artifact in nonempty(),
            detector in nonempty(),
            image in sha256_hex(),
            section in sha256_hex(),
        ) {
            let va = figure_candidate_vid(&rev_a, &artifact, &detector, &image, &section).unwrap();
            let va_again = figure_candidate_vid(&rev_a, &artifact, &detector, &image, &section).unwrap();
            prop_assert_eq!(&va, &va_again, "deterministic identity");
            prop_assert_eq!(va.len(), 64, "VID must be 64 hex chars");
            if rev_a != rev_b {
                let vb = figure_candidate_vid(&rev_b, &artifact, &detector, &image, &section).unwrap();
                prop_assert_ne!(&va, &vb, "revisions must produce disjoint candidates");
            }
        }

        /// Every identity component is bound: perturbing any of them changes the VID.
        #[test]
        fn prop_candidate_vid_binds_every_identity_component(
            rev in sha256_hex(),
            artifact in nonempty(),
            detector in nonempty(),
            image in sha256_hex(),
            section in sha256_hex(),
        ) {
            let base = figure_candidate_vid(&rev, &artifact, &detector, &image, &section).unwrap();
            let rev_other = flip_hex(&rev);
            let image_other = flip_hex(&image);
            let section_other = flip_hex(&section);
            prop_assert_ne!(&base, &figure_candidate_vid(&rev_other, &artifact, &detector, &image, &section).unwrap(), "revision bound");
            prop_assert_ne!(&base, &figure_candidate_vid(&rev, &format!("{artifact}x"), &detector, &image, &section).unwrap(), "artifact bound");
            prop_assert_ne!(&base, &figure_candidate_vid(&rev, &artifact, &format!("{detector}x"), &image, &section).unwrap(), "detector bound");
            prop_assert_ne!(&base, &figure_candidate_vid(&rev, &artifact, &detector, &image_other, &section).unwrap(), "image bound");
            prop_assert_ne!(&base, &figure_candidate_vid(&rev, &artifact, &detector, &image, &section_other).unwrap(), "section bound");
        }

        /// A valid candidate validates; unlocking authority is always rejected.
        #[test]
        fn prop_candidate_validate_rejects_authority_unlock(
            rev in sha256_hex(),
            artifact in nonempty(),
            detector in nonempty(),
            image in sha256_hex(),
            section in sha256_hex(),
        ) {
            let candidate = valid_candidate(&rev, &artifact, &detector, &image, &section);
            prop_assert!(candidate.validate().is_ok(), "valid candidate must validate");
            let mut unlocked = candidate.clone();
            unlocked.authoritative = true;
            prop_assert_eq!(unlocked.validate(), Err(FigureCandidateError::AuthorityViolation));
            let mut unlocked = candidate;
            unlocked.import_eligible = !unlocked.import_eligible;
            prop_assert_eq!(unlocked.validate(), Err(FigureCandidateError::AuthorityViolation));
        }
    }

    /// Flip the first hex character so the digest remains 64 hex but changes.
    fn flip_hex(value: &str) -> String {
        let first = value.chars().next().unwrap();
        let flipped = if first == '0' { '1' } else { '0' };
        format!("{flipped}{}", &value[1..])
    }
}
