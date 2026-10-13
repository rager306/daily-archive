//! Candidate-only per-document captures. No journal or production-state writes.

use da_application::parse_policy_pair::ParseArm;
use da_application::parse_policy_table::PolicyOutcomeEvidence;
use da_domain::fsm_policy::ArticleFeatures;
use serde::{Deserialize, Serialize};

pub const CAPTURE_SCHEMA: &str = "policy-corpus-capture.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub schema_version: String,
    pub paper_id: String,
    pub source: Artifact,
    pub source_bytes: u64,
    pub features: Option<ArticleFeatures>,
    pub features_error: Option<String>,
    pub exporter: ExporterIdentity,
    pub reference: ReferenceCapture,
    pub arms: [ArmCapture; 2],
    pub candidate_only: bool,
    pub deployed: bool,
    pub import_eligible: bool,
    pub graph_write: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExporterIdentity {
    pub script: Artifact,
    pub furniture_policy: Artifact,
    pub python: String,
    pub python_version: Option<String>,
    pub pymupdf_version: Option<String>,
    pub observations: Option<Artifact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCapture {
    pub tool: String,
    pub version: Option<String>,
    pub artifact: Option<Artifact>,
    pub char_count: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Success,
    NotApplicable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmCapture {
    pub arm: ParseArm,
    pub decision_code: String,
    pub grammar_profile: String,
    pub backend_profile: Option<String>,
    pub status: CaptureStatus,
    pub error: Option<String>,
    pub article: Option<Artifact>,
    pub evidence: Option<PolicyOutcomeEvidence>,
    pub backend_version: Option<String>,
    pub latency_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
        let bytes = serde_json::to_vec(value).unwrap();
        assert_eq!(value, &serde_json::from_slice::<T>(&bytes).unwrap());
    }

    #[test]
    fn capture_carriers_roundtrip_without_inventing_absent_evidence() {
        let artifact = Artifact {
            path: "reference/pdftotext.txt".into(),
            sha256: "a".repeat(64),
        };
        let reference = ReferenceCapture {
            tool: "pdftotext".into(),
            version: Some("test-version".into()),
            artifact: Some(artifact.clone()),
            char_count: Some(42),
            error: None,
        };
        let exporter = ExporterIdentity {
            script: artifact.clone(),
            furniture_policy: artifact.clone(),
            python: "python3".into(),
            python_version: Some("3.test".into()),
            pymupdf_version: Some("1.test".into()),
            observations: Some(artifact.clone()),
        };
        let missing = ArmCapture {
            arm: ParseArm::Toc,
            decision_code: "page_outline".into(),
            grammar_profile: "page_outline".into(),
            backend_profile: None,
            status: CaptureStatus::NotApplicable,
            error: Some("empty_toc".into()),
            article: None,
            evidence: None,
            backend_version: Some("1.test".into()),
            latency_ms: 17,
        };
        let failed = ArmCapture {
            arm: ParseArm::Grobid,
            status: CaptureStatus::Failed,
            decision_code: "xy_cut".into(),
            grammar_profile: "arxiv_pdf_dot".into(),
            backend_profile: Some("structural".into()),
            error: Some("probe unavailable".into()),
            backend_version: None,
            ..missing.clone()
        };
        let capture = Capture {
            schema_version: CAPTURE_SCHEMA.into(),
            paper_id: "paper".into(),
            source: artifact.clone(),
            source_bytes: 99,
            features: None,
            features_error: Some("not measured".into()),
            exporter: exporter.clone(),
            reference: reference.clone(),
            arms: [missing.clone(), failed],
            candidate_only: true,
            deployed: false,
            import_eligible: false,
            graph_write: false,
        };
        roundtrip(&artifact);
        roundtrip(&reference);
        roundtrip(&exporter);
        roundtrip(&missing);
        for status in [
            CaptureStatus::Success,
            CaptureStatus::NotApplicable,
            CaptureStatus::Failed,
        ] {
            roundtrip(&status);
        }
        roundtrip(&capture);
    }
}
