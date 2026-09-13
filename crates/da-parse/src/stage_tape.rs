//! Typed stage tape and its verifier (M506/S01/T04).
//!
//! A tape is migration evidence, so it must be *checkable*, not merely stored.
//! Three failures would otherwise be invisible:
//!
//! - **source substitution** — the tape names a PDF whose bytes no longer hash
//!   to the recorded digest, so the evidence now describes a different input;
//! - **dump substitution** — the recorded dump digest and the artifact on disk
//!   disagree, so a reader would trust content that was never captured;
//! - **release drift** — the tape was recorded against one GROBID release while
//!   the live service now answers with another, which silently reinterprets
//!   every feature and label in the dump.
//!
//! Capture states are explicit rather than inferred, because "the dump had no
//! section for this model" must never be read as "the model did not run":
//! `uncaptured_empty` and `filtered` both mean the tape does not prove absence.
//!
//! The verifier is pure: it compares a tape against re-measured facts and
//! returns typed findings. Nothing here performs I/O, so the rules are
//! testable without a service, a filesystem, or a release.

use serde::{Deserialize, Serialize};

/// Version of the tape schema written by the recorder.
pub const STAGE_TAPE_SCHEMA_VERSION: &str = "grobid-stage-tape.v1";

/// What the recorder actually obtained, stated explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    /// A dump came back with content and no filter was applied.
    Captured,
    /// A successful response that carried no bytes. Absence of evidence, not
    /// evidence of absence.
    UncapturedEmpty,
    /// A `models` filter was applied, so the dump is a subset by construction.
    Filtered,
    /// The request failed; the typed code and phase are recorded alongside.
    Failed,
}

impl CaptureState {
    /// Stable wire name, identical to the serde form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::UncapturedEmpty => "uncaptured_empty",
            Self::Filtered => "filtered",
            Self::Failed => "failed",
        }
    }

    /// Whether this state may be used as evidence that the cascade ran.
    ///
    /// Only `captured` and `filtered` carry content. A reader that needs to
    /// prove a model ran must additionally check [`StageTape::sections`].
    pub const fn carries_content(self) -> bool {
        matches!(self, Self::Captured | Self::Filtered)
    }
}

/// Per-model invocation count: the coverage view a later slice compares against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCoverage {
    pub model: String,
    pub invocations: usize,
}

/// A recorded debug capture, bound to the exact input and release it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageTape {
    pub schema_version: String,
    pub endpoint: String,
    pub paper_id: String,
    pub source_pdf: String,
    pub source_sha256: String,
    pub backend_version: String,
    pub models_filter: Option<String>,
    pub capture_state: CaptureState,
    pub dump_bytes: usize,
    pub dump_sha256: Option<String>,
    /// Model sections found: invocations, not distinct models.
    pub sections: usize,
    /// Distinct models exercised, derived from `models_observed`.
    pub distinct_models: usize,
    pub models_observed: Vec<ModelCoverage>,
    pub failure_code: Option<String>,
    pub failure_phase: Option<String>,
    pub notes: Vec<String>,
}

impl StageTape {
    /// Build a tape from one capture, deriving coverage from the dump itself.
    ///
    /// Deriving here rather than in the caller means every recorder (and every
    /// future one) reports coverage the same way, and the derivation is
    /// covered by these tests instead of by whichever example happens to run.
    #[allow(clippy::too_many_arguments)]
    pub fn from_capture(
        endpoint: &str,
        paper_id: &str,
        source_pdf: &str,
        source_sha256: &str,
        backend_version: &str,
        models_filter: Option<&str>,
        dump: &[u8],
        failure: Option<(&str, &str)>,
        notes: Vec<String>,
    ) -> Self {
        let dump_text = String::from_utf8_lossy(dump);
        let models_observed = crate::debug_tape::model_invocation_counts(&dump_text)
            .into_iter()
            .map(|(model, invocations)| ModelCoverage { model, invocations })
            .collect::<Vec<_>>();
        let sections = crate::debug_tape::parse_debug_sections(&dump_text).len();

        let capture_state = match failure {
            Some(_) => CaptureState::Failed,
            None if dump.is_empty() => CaptureState::UncapturedEmpty,
            None if models_filter.is_some() => CaptureState::Filtered,
            None => CaptureState::Captured,
        };

        Self {
            schema_version: STAGE_TAPE_SCHEMA_VERSION.to_string(),
            endpoint: endpoint.to_string(),
            paper_id: paper_id.to_string(),
            source_pdf: source_pdf.to_string(),
            source_sha256: source_sha256.to_string(),
            backend_version: backend_version.to_string(),
            models_filter: models_filter.map(str::to_string),
            capture_state,
            dump_bytes: dump.len(),
            dump_sha256: (!dump.is_empty()).then(|| sha256_hex(dump)),
            sections,
            distinct_models: models_observed.len(),
            models_observed,
            failure_code: failure.map(|(code, _)| code.to_string()),
            failure_phase: failure.map(|(_, phase)| phase.to_string()),
            notes,
        }
    }

    /// Invocations recorded for one model, or `None` when it does not appear.
    ///
    /// `None` means "no evidence in this tape", never "the model did not run":
    /// a filtered tape omits models by construction.
    pub fn invocations_of(&self, model: &str) -> Option<usize> {
        self.models_observed
            .iter()
            .find(|entry| entry.model == model)
            .map(|entry| entry.invocations)
    }
}

/// Facts re-measured at verification time, independent of the tape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeObservation {
    pub source_sha256: String,
    pub dump_sha256: Option<String>,
    pub probed_backend_version: String,
}

/// A typed reason a tape cannot be trusted as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapeFinding {
    UnsupportedSchema { found: String },
    SourceDigestMismatch { expected: String, observed: String },
    DumpDigestMismatch { expected: String, observed: String },
    DumpMissingButDigestRecorded { expected: String },
    DumpPresentButNoDigestRecorded,
    BackendVersionMismatch { expected: String, observed: String },
    FailedCapture { code: String, phase: String },
    NoSectionsCaptured { state: CaptureState },
    CoverageInconsistent { sections: usize, counted: usize },
}

impl TapeFinding {
    /// Short stable label, suitable for logs and assertions.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedSchema { .. } => "unsupported_schema",
            Self::SourceDigestMismatch { .. } => "source_digest_mismatch",
            Self::DumpDigestMismatch { .. } => "dump_digest_mismatch",
            Self::DumpMissingButDigestRecorded { .. } => "dump_missing",
            Self::DumpPresentButNoDigestRecorded => "dump_digest_absent",
            Self::BackendVersionMismatch { .. } => "backend_version_mismatch",
            Self::FailedCapture { .. } => "failed_capture",
            Self::NoSectionsCaptured { .. } => "no_sections",
            Self::CoverageInconsistent { .. } => "coverage_inconsistent",
        }
    }
}

/// Compare a tape against re-measured facts.
///
/// Returns every finding rather than the first: a caller diagnosing a tape
/// needs the whole picture, and short-circuiting would hide a substitution
/// behind an unrelated failure.
pub fn verify_tape(tape: &StageTape, observed: &TapeObservation) -> Vec<TapeFinding> {
    let mut findings = Vec::new();

    if tape.schema_version != STAGE_TAPE_SCHEMA_VERSION {
        findings.push(TapeFinding::UnsupportedSchema {
            found: tape.schema_version.clone(),
        });
    }

    if tape.source_sha256 != observed.source_sha256 {
        findings.push(TapeFinding::SourceDigestMismatch {
            expected: tape.source_sha256.clone(),
            observed: observed.source_sha256.clone(),
        });
    }

    match (&tape.dump_sha256, &observed.dump_sha256) {
        // The tape claims a dump, but the artifact is gone: the evidence cannot
        // be re-derived, so the tape is no longer usable even though its bytes
        // field still claims a size.
        (Some(expected), None) => {
            findings.push(TapeFinding::DumpMissingButDigestRecorded {
                expected: expected.clone(),
            });
        }
        // A dump exists with no recorded digest: nothing can bind it to the
        // tape, so it must not be treated as verified content.
        (None, Some(_)) => findings.push(TapeFinding::DumpPresentButNoDigestRecorded),
        (Some(expected), Some(actual)) if expected != actual => {
            findings.push(TapeFinding::DumpDigestMismatch {
                expected: expected.clone(),
                observed: actual.clone(),
            });
        }
        _ => {}
    }

    // Release drift is checked whenever the tape recorded a version, including
    // failed captures: a failed tape recorded against another release still
    // misdescribes the cascade that produced the failure.
    if !tape.backend_version.is_empty() && tape.backend_version != observed.probed_backend_version {
        findings.push(TapeFinding::BackendVersionMismatch {
            expected: tape.backend_version.clone(),
            observed: observed.probed_backend_version.clone(),
        });
    }

    if let (Some(code), Some(phase)) = (&tape.failure_code, &tape.failure_phase) {
        findings.push(TapeFinding::FailedCapture {
            code: code.clone(),
            phase: phase.clone(),
        });
    }

    if tape.capture_state.carries_content() && tape.sections == 0 {
        findings.push(TapeFinding::NoSectionsCaptured {
            state: tape.capture_state,
        });
    }

    let counted: usize = tape.models_observed.iter().map(|m| m.invocations).sum();
    if counted != tape.sections || tape.models_observed.len() != tape.distinct_models {
        findings.push(TapeFinding::CoverageInconsistent {
            sections: tape.sections,
            counted,
        });
    }

    findings
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureState, STAGE_TAPE_SCHEMA_VERSION, StageTape, TapeFinding, TapeObservation,
        verify_tape,
    };

    const DUMP: &str = "=== model: segmentation ===\ns1\tA\n=== model: date (occurrence 1 of 2) ===\nd1\tB\n=== model: date (occurrence 2 of 2) ===\nd2\tC\n";

    fn tape() -> StageTape {
        StageTape::from_capture(
            "fulltext",
            "paper",
            "data/paper.pdf",
            "aaaa",
            "0.9.1",
            None,
            DUMP.as_bytes(),
            None,
            vec!["note".to_string()],
        )
    }

    fn clean_observation(from: &StageTape) -> TapeObservation {
        TapeObservation {
            source_sha256: from.source_sha256.clone(),
            dump_sha256: from.dump_sha256.clone(),
            probed_backend_version: from.backend_version.clone(),
        }
    }

    #[test]
    fn a_faithful_tape_verifies_with_no_findings() {
        let tape = tape();
        assert!(verify_tape(&tape, &clean_observation(&tape)).is_empty());
    }

    #[test]
    fn construction_derives_coverage_from_the_dump_not_from_the_caller() {
        let tape = tape();
        assert_eq!(tape.schema_version, STAGE_TAPE_SCHEMA_VERSION);
        assert_eq!(tape.capture_state, CaptureState::Captured);
        assert_eq!(tape.sections, 3);
        assert_eq!(tape.distinct_models, 2);
        assert_eq!(tape.invocations_of("date"), Some(2));
        assert_eq!(tape.invocations_of("segmentation"), Some(1));
        // Absence of a model in the tape is not a claim that it did not run.
        assert_eq!(tape.invocations_of("citation"), None);
    }

    #[test]
    fn source_substitution_is_detected() {
        let tape = tape();
        let mut observed = clean_observation(&tape);
        observed.source_sha256 = "bbbb".to_string();
        assert_eq!(
            verify_tape(&tape, &observed),
            vec![TapeFinding::SourceDigestMismatch {
                expected: "aaaa".to_string(),
                observed: "bbbb".to_string(),
            }]
        );
    }

    #[test]
    fn dump_substitution_and_missing_dump_are_both_detected() {
        let tape = tape();
        let recorded = tape.dump_sha256.clone().expect("dump digest");

        let mut substituted = clean_observation(&tape);
        substituted.dump_sha256 = Some("cccc".to_string());
        assert_eq!(
            verify_tape(&tape, &substituted),
            vec![TapeFinding::DumpDigestMismatch {
                expected: recorded.clone(),
                observed: "cccc".to_string(),
            }]
        );

        let mut missing = clean_observation(&tape);
        missing.dump_sha256 = None;
        assert_eq!(
            verify_tape(&tape, &missing),
            vec![TapeFinding::DumpMissingButDigestRecorded { expected: recorded }]
        );
    }

    #[test]
    fn release_drift_is_detected_independently_of_the_other_checks() {
        let tape = tape();
        let mut observed = clean_observation(&tape);
        observed.probed_backend_version = "0.9.2".to_string();
        assert_eq!(
            verify_tape(&tape, &observed),
            vec![TapeFinding::BackendVersionMismatch {
                expected: "0.9.1".to_string(),
                observed: "0.9.2".to_string(),
            }]
        );
    }

    #[test]
    fn a_failed_capture_is_reported_as_a_finding_rather_than_passing_quietly() {
        let tape = StageTape::from_capture(
            "fulltext",
            "paper",
            "data/paper.pdf",
            "aaaa",
            "0.9.1",
            Some("definitely-not-a-model"),
            &[],
            Some(("invalid_request", "await_response")),
            Vec::new(),
        );
        assert_eq!(tape.capture_state, CaptureState::Failed);
        let findings = verify_tape(&tape, &clean_observation(&tape));
        assert!(
            findings.contains(&TapeFinding::FailedCapture {
                code: "invalid_request".to_string(),
                phase: "await_response".to_string(),
            }),
            "failed capture must surface: {findings:?}"
        );
    }

    #[test]
    fn content_that_carries_no_section_is_flagged_as_no_evidence() {
        // A 200 response with bytes but no model header: the state says content,
        // yet nothing was captured. This is the incomplete-capture negative.
        let tape = StageTape::from_capture(
            "fulltext",
            "paper",
            "data/paper.pdf",
            "aaaa",
            "0.9.1",
            None,
            b"plain text, no model headers\n".as_slice(),
            None,
            Vec::new(),
        );
        assert_eq!(tape.capture_state, CaptureState::Captured);
        assert_eq!(tape.sections, 0);
        let findings = verify_tape(&tape, &clean_observation(&tape));
        assert_eq!(
            findings,
            vec![TapeFinding::NoSectionsCaptured {
                state: CaptureState::Captured
            }]
        );
    }

    #[test]
    fn an_empty_success_is_uncaptured_rather_than_captured() {
        let tape = StageTape::from_capture(
            "fulltext",
            "paper",
            "data/paper.pdf",
            "aaaa",
            "0.9.1",
            None,
            &[],
            None,
            Vec::new(),
        );
        assert_eq!(tape.capture_state, CaptureState::UncapturedEmpty);
        assert!(verify_tape(&tape, &clean_observation(&tape)).is_empty());
    }

    #[test]
    fn a_filtered_tape_is_never_mistaken_for_a_full_one() {
        let tape = StageTape::from_capture(
            "fulltext",
            "paper",
            "data/paper.pdf",
            "aaaa",
            "0.9.1",
            Some("segmentation"),
            b"=== model: segmentation ===\ns1\tA\n".as_slice(),
            None,
            Vec::new(),
        );
        assert_eq!(tape.capture_state, CaptureState::Filtered);
        assert!(
            !verify_tape(&tape, &clean_observation(&tape))
                .iter()
                .any(|finding| finding.code() == "no_sections")
        );
    }

    #[test]
    fn every_finding_carries_a_stable_code() {
        let cases = [
            TapeFinding::UnsupportedSchema {
                found: "x".to_string(),
            },
            TapeFinding::DumpPresentButNoDigestRecorded,
            TapeFinding::CoverageInconsistent {
                sections: 1,
                counted: 2,
            },
        ];
        for finding in cases {
            assert!(!finding.code().is_empty());
        }
    }
}
