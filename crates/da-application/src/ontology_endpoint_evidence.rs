//! Validated evidence emitted by the bounded ontology endpoint canary.
//!
//! This DTO is application-owned so endpoint acceptance semantics are testable
//! without depending on CLI JSON glue. It binds a canonical runtime graph
//! observation and remains permanently ineligible for D127 import authority.

use da_domain::ContentDigest;
use serde::{Deserialize, Serialize};

pub const ENDPOINT_EVIDENCE_SCHEMA_VERSION: &str = "ontology-v2-endpoint-canary.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OntologyEndpointEvidence {
    pub schema_version: String,
    pub paper_id: String,
    pub graph_semantics_sha256: String,
    pub node_schemas_sha256: String,
    pub edge_contracts_sha256: String,
    pub finalize_policy_sha256: String,
    pub semantic_tasks_source_sha256: String,
    pub first_cites_edges_written: usize,
    pub replay_cites_edges_written: usize,
    pub citation_nodes: usize,
    pub quality_signal_nodes: usize,
    pub cites_edges: usize,
    pub quality_signal_edges: usize,
    pub forbidden_paper_to_paper_cites: usize,
    pub forbidden_summary_to_quality_signal: usize,
    pub temporally_stamped_fact_edges: usize,
    pub endpoint_triples: Vec<String>,
    pub endpoint_replay_stable: bool,
    pub full_graph_replay_stable: bool,
    pub semantic_task_count: usize,
    pub semantic_tasks_passed: bool,
    pub finalize_passed: bool,
    pub finalize_schema_violations: usize,
    pub finalize_unknown_edge_types: Vec<String>,
    pub finalize_blockers: Vec<String>,
    pub import_eligible: bool,
    pub report_sha256: String,
}

impl OntologyEndpointEvidence {
    pub fn seal(mut self) -> Result<Self, OntologyEndpointEvidenceError> {
        self.endpoint_triples.sort();
        self.endpoint_triples.dedup();
        self.finalize_unknown_edge_types.sort();
        self.finalize_unknown_edge_types.dedup();
        self.finalize_blockers.sort();
        self.finalize_blockers.dedup();
        self.report_sha256 = report_digest(&self);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), OntologyEndpointEvidenceError> {
        if self.schema_version != ENDPOINT_EVIDENCE_SCHEMA_VERSION
            || self.paper_id.trim().is_empty()
            || self.import_eligible
            || !valid_digest(&self.graph_semantics_sha256)
            || !valid_digest(&self.node_schemas_sha256)
            || !valid_digest(&self.edge_contracts_sha256)
            || !valid_digest(&self.finalize_policy_sha256)
            || !valid_digest(&self.semantic_tasks_source_sha256)
        {
            return Err(OntologyEndpointEvidenceError::InvalidEvidence(
                "schema, identity, graph digest, or D127 authority drift".into(),
            ));
        }
        if self.first_cites_edges_written == 0
            || self.replay_cites_edges_written != 0
            || self.citation_nodes == 0
            || self.quality_signal_nodes == 0
            || self.cites_edges != self.citation_nodes
            || self.quality_signal_edges != self.quality_signal_nodes
            || self.forbidden_paper_to_paper_cites != 0
            || self.forbidden_summary_to_quality_signal != 0
            || self.temporally_stamped_fact_edges != self.cites_edges + self.quality_signal_edges
            || !self.endpoint_replay_stable
            || !self.full_graph_replay_stable
            || self.semantic_task_count == 0
            || !self.semantic_tasks_passed
            || self.finalize_schema_violations != 0
            || !self.finalize_unknown_edge_types.is_empty()
            || (self.finalize_passed && !self.finalize_blockers.is_empty())
            || (!self.finalize_passed
                && (self.finalize_blockers.is_empty()
                    || self
                        .finalize_blockers
                        .iter()
                        .any(|code| !code.starts_with("required_stage_not_passed:"))))
        {
            return Err(OntologyEndpointEvidenceError::InvalidEvidence(
                "endpoint, temporal, or replay invariant failed".into(),
            ));
        }
        let has_cites = self
            .endpoint_triples
            .iter()
            .any(|triple| triple.starts_with("CITES:Paper->Citation="));
        let has_quality = self
            .endpoint_triples
            .iter()
            .any(|triple| triple.starts_with("hasQualitySignal:Paper->QualitySignal="));
        let forbidden_shape = self.endpoint_triples.iter().any(|triple| {
            triple.starts_with("CITES:Paper->Paper=")
                || triple.starts_with("hasSummary:Paper->QualitySignal=")
        });
        if !has_cites || !has_quality || forbidden_shape {
            return Err(OntologyEndpointEvidenceError::InvalidEvidence(
                "required endpoint triples are absent or forbidden triples are present".into(),
            ));
        }
        if self.report_sha256 != report_digest(self) {
            return Err(OntologyEndpointEvidenceError::InvalidEvidence(
                "report digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OntologyEndpointEvidenceError {
    #[error("invalid ontology endpoint evidence: {0}")]
    InvalidEvidence(String),
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn report_digest(report: &OntologyEndpointEvidence) -> String {
    let mut copy = report.clone();
    copy.report_sha256.clear();
    let bytes = serde_json::to_vec(&copy).expect("endpoint evidence is serializable");
    format!("sha256:{}", ContentDigest::of_bytes(&bytes).as_str())
}
