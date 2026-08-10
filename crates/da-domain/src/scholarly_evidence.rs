//! Multi-provider scholarly evidence reconciliation (M371/S11).
//!
//! Pure domain: OpenAlex / Semantic Scholar / local TeX bibliography candidates
//! reconcile into a durable report with visible conflicts. Candidate-only —
//! never unlocks D127 and never promotes provider payloads to graph truth.

use crate::ContentDigest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHOLARLY_EVIDENCE_SCHEMA_VERSION: &str = "scholarly-evidence-reconcile.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScholarlyFindingSeverity {
    Info,
    ReviewRequired,
    MustFilter,
    HardConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ScholarlyFinding {
    pub severity: ScholarlyFindingSeverity,
    pub class: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderObservation {
    pub provider: String,
    pub status: String,
    pub payload_sha256: Option<ContentDigest>,
    pub work_id: Option<String>,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub cited_by_count: Option<u64>,
    pub referenced_works_count: Option<u64>,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalBibObservation {
    pub candidate_id: ContentDigest,
    pub entry_key: String,
    pub entry_type: String,
    pub title: Option<String>,
    pub doi: Option<String>,
    pub eprint: Option<String>,
    pub source_member: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTexObservation {
    pub analysis_sha256: ContentDigest,
    pub archive_sha256: ContentDigest,
    pub root_document: Option<String>,
    pub status: String,
    pub local_reference_denominator: Option<usize>,
    pub bibliography: Vec<LocalBibObservation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScholarlyEvidenceInput {
    pub article_id: String,
    pub title: Option<String>,
    pub providers: Vec<ProviderObservation>,
    pub tex: Option<LocalTexObservation>,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScholarlyReconcileVerdict {
    /// Providers and local TeX agree on denominators / identities within policy.
    Converged,
    /// Conflicts or missing providers require review (candidate-only).
    NeedsReview,
    /// Hard authority/D127 or empty article identity.
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedBibliographyLink {
    pub local_entry_key: String,
    pub local_candidate_id: ContentDigest,
    pub provider: String,
    pub match_kind: String,
    pub provider_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScholarlyEvidenceReport {
    pub schema_version: String,
    pub article_id: String,
    pub title: Option<String>,
    pub providers: Vec<ProviderObservation>,
    pub tex: Option<LocalTexObservation>,
    pub findings: Vec<ScholarlyFinding>,
    pub matched_bibliography: Vec<MatchedBibliographyLink>,
    pub unmatched_local_bibliography: usize,
    pub provider_reference_denominators: BTreeMap<String, Option<usize>>,
    pub local_reference_denominator: Option<usize>,
    pub verdict: ScholarlyReconcileVerdict,
    pub import_eligible: bool,
    pub report_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScholarlyEvidenceError {
    #[error("invalid scholarly evidence input: {0}")]
    InvalidInput(String),
    #[error("invalid scholarly evidence report: {0}")]
    InvalidReport(String),
}

impl ScholarlyEvidenceReport {
    pub fn validate(&self) -> Result<(), ScholarlyEvidenceError> {
        if self.schema_version != SCHOLARLY_EVIDENCE_SCHEMA_VERSION {
            return Err(ScholarlyEvidenceError::InvalidReport(
                "schema version mismatch".into(),
            ));
        }
        if self.import_eligible {
            return Err(ScholarlyEvidenceError::InvalidReport(
                "import_eligible must be false".into(),
            ));
        }
        if self.report_sha256 != report_digest(self) {
            return Err(ScholarlyEvidenceError::InvalidReport(
                "report digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Pure multi-provider scholarly reconcile.
pub fn reconcile_scholarly_evidence(
    mut input: ScholarlyEvidenceInput,
) -> Result<ScholarlyEvidenceReport, ScholarlyEvidenceError> {
    if input.article_id.trim().is_empty() {
        return Err(ScholarlyEvidenceError::InvalidInput(
            "article_id must not be empty".into(),
        ));
    }
    if input.import_eligible {
        return Err(ScholarlyEvidenceError::InvalidInput(
            "import_eligible must be false".into(),
        ));
    }
    input.providers.sort_by(|a, b| a.provider.cmp(&b.provider));
    for provider in &input.providers {
        if provider.provider.trim().is_empty() {
            return Err(ScholarlyEvidenceError::InvalidInput(
                "provider name must not be empty".into(),
            ));
        }
    }

    let mut findings = Vec::new();
    let mut provider_reference_denominators = BTreeMap::new();
    for provider in &input.providers {
        provider_reference_denominators.insert(
            provider.provider.clone(),
            provider.referenced_works_count.map(|n| n as usize),
        );
        if provider.status == "rate_limited" {
            findings.push(ScholarlyFinding {
                severity: ScholarlyFindingSeverity::ReviewRequired,
                class: "provider_rate_limited".into(),
                subject: provider.provider.clone(),
                detail: format!(
                    "Provider {} is rate-limited; observed counts remain candidate-only",
                    provider.provider
                ),
            });
        }
        if provider.status == "error" || provider.status == "failed" {
            findings.push(ScholarlyFinding {
                severity: ScholarlyFindingSeverity::ReviewRequired,
                class: "provider_fetch_failed".into(),
                subject: provider.provider.clone(),
                detail: format!(
                    "Provider {} fetch status={}",
                    provider.provider, provider.status
                ),
            });
        }
    }

    let local_reference_denominator = input
        .tex
        .as_ref()
        .and_then(|tex| tex.local_reference_denominator)
        .or_else(|| {
            input
                .tex
                .as_ref()
                .map(|tex| tex.bibliography.len())
                .filter(|n| *n > 0)
        });

    if let Some(tex) = &input.tex {
        if tex.status == "conflict" || tex.status == "reject" {
            findings.push(ScholarlyFinding {
                severity: if tex.status == "reject" {
                    ScholarlyFindingSeverity::HardConflict
                } else {
                    ScholarlyFindingSeverity::ReviewRequired
                },
                class: format!("tex_status_{}", tex.status),
                subject: input.article_id.clone(),
                detail: format!(
                    "Local TeX analysis status={} with {} diagnostics",
                    tex.status,
                    tex.diagnostics.len()
                ),
            });
        }
        for diagnostic in &tex.diagnostics {
            if diagnostic.starts_with("provider_denominator_conflict:") {
                findings.push(ScholarlyFinding {
                    severity: ScholarlyFindingSeverity::ReviewRequired,
                    class: "citation_denominator_conflict".into(),
                    subject: diagnostic.clone(),
                    detail: format!(
                        "TeX diagnostic {diagnostic}; local denominator={:?}",
                        local_reference_denominator
                    ),
                });
            }
        }
    } else {
        findings.push(ScholarlyFinding {
            severity: ScholarlyFindingSeverity::Info,
            class: "tex_analysis_absent".into(),
            subject: input.article_id.clone(),
            detail: "No local TeX analysis observation injected".into(),
        });
    }

    // Cross-provider denominator conflicts vs local TeX.
    if let Some(local) = local_reference_denominator {
        for (provider, count) in &provider_reference_denominators {
            match count {
                Some(n) if *n != local => {
                    findings.push(ScholarlyFinding {
                        severity: ScholarlyFindingSeverity::ReviewRequired,
                        class: "citation_denominator_conflict".into(),
                        subject: provider.clone(),
                        detail: format!(
                            "Provider {provider} references={n} vs local TeX denominator={local}"
                        ),
                    });
                }
                Some(0) if local > 0 => {
                    findings.push(ScholarlyFinding {
                        severity: ScholarlyFindingSeverity::ReviewRequired,
                        class: "provider_zero_references_with_local_bib".into(),
                        subject: provider.clone(),
                        detail: format!(
                            "Provider {provider} reports 0 referenced works while local TeX has {local}"
                        ),
                    });
                }
                None => {
                    findings.push(ScholarlyFinding {
                        severity: ScholarlyFindingSeverity::Info,
                        class: "provider_reference_count_unknown".into(),
                        subject: provider.clone(),
                        detail: format!(
                            "Provider {provider} has no referenced_works_count observation"
                        ),
                    });
                }
                _ => {}
            }
        }
    }

    // Identity conflicts among fetched providers (title/doi).
    let fetched: Vec<_> = input
        .providers
        .iter()
        .filter(|p| p.status == "fetched")
        .collect();
    if fetched.len() >= 2 {
        let dois: BTreeSet<_> = fetched
            .iter()
            .filter_map(|p| p.doi.as_ref().map(|d| normalize_doi(d)))
            .filter(|d| !d.is_empty())
            .collect();
        if dois.len() > 1 {
            findings.push(ScholarlyFinding {
                severity: ScholarlyFindingSeverity::HardConflict,
                class: "provider_doi_conflict".into(),
                subject: input.article_id.clone(),
                detail: format!("Multiple DOIs across providers: {dois:?}"),
            });
        }
    }

    // Match local bibliography to provider keys by DOI / eprint / normalized title.
    let mut matched = Vec::new();
    let mut matched_local = BTreeSet::new();
    if let Some(tex) = &input.tex {
        // Provider-side match keys from OpenAlex-like payloads are limited at
        // article level; per-reference matching is only possible when local DOI/eprint
        // equals article DOI/eprint or when future per-ref provider lists are injected.
        let article_dois: BTreeSet<_> = input
            .providers
            .iter()
            .filter_map(|p| p.doi.as_ref().map(|d| normalize_doi(d)))
            .filter(|d| !d.is_empty())
            .collect();
        for bib in &tex.bibliography {
            if let Some(doi) = bib.doi.as_ref().map(|d| normalize_doi(d))
                && article_dois.contains(&doi)
            {
                // Local bib entry shares article DOI — unusual self-cite / contamination.
                findings.push(ScholarlyFinding {
                    severity: ScholarlyFindingSeverity::MustFilter,
                    class: "local_bib_shares_article_doi".into(),
                    subject: bib.entry_key.clone(),
                    detail: format!(
                        "Bibliography entry {} DOI equals article DOI {doi}",
                        bib.entry_key
                    ),
                });
            }
            // Cross-link local DOI presence as candidate identity completeness.
            if bib.doi.is_some() || bib.eprint.is_some() {
                matched.push(MatchedBibliographyLink {
                    local_entry_key: bib.entry_key.clone(),
                    local_candidate_id: bib.candidate_id.clone(),
                    provider: "local_tex".into(),
                    match_kind: if bib.doi.is_some() {
                        "local_doi_present".into()
                    } else {
                        "local_eprint_present".into()
                    },
                    provider_key: bib
                        .doi
                        .clone()
                        .or_else(|| bib.eprint.clone())
                        .unwrap_or_default(),
                });
                matched_local.insert(bib.entry_key.clone());
            }
        }
    }

    let unmatched_local_bibliography = input
        .tex
        .as_ref()
        .map(|tex| {
            tex.bibliography
                .iter()
                .filter(|b| !matched_local.contains(&b.entry_key))
                .count()
        })
        .unwrap_or(0);
    if unmatched_local_bibliography > 0 {
        findings.push(ScholarlyFinding {
            severity: ScholarlyFindingSeverity::Info,
            class: "local_bib_without_strong_id".into(),
            subject: input.article_id.clone(),
            detail: format!(
                "{unmatched_local_bibliography} local bibliography entries lack DOI/eprint"
            ),
        });
    }

    findings.sort();
    findings.dedup();
    matched.sort_by(|a, b| a.local_entry_key.cmp(&b.local_entry_key));

    let verdict = if findings
        .iter()
        .any(|f| f.severity == ScholarlyFindingSeverity::HardConflict)
    {
        ScholarlyReconcileVerdict::Reject
    } else if findings.iter().any(|f| {
        matches!(
            f.severity,
            ScholarlyFindingSeverity::ReviewRequired | ScholarlyFindingSeverity::MustFilter
        )
    }) {
        ScholarlyReconcileVerdict::NeedsReview
    } else {
        ScholarlyReconcileVerdict::Converged
    };

    let mut report = ScholarlyEvidenceReport {
        schema_version: SCHOLARLY_EVIDENCE_SCHEMA_VERSION.into(),
        article_id: input.article_id,
        title: input.title,
        providers: input.providers,
        tex: input.tex,
        findings,
        matched_bibliography: matched,
        unmatched_local_bibliography,
        provider_reference_denominators,
        local_reference_denominator,
        verdict,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = report_digest(&report);
    report.validate()?;
    Ok(report)
}

fn normalize_doi(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    lower
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_start_matches("doi:")
        .to_string()
}

fn report_digest(report: &ScholarlyEvidenceReport) -> String {
    let mut clone = report.clone();
    clone.report_sha256 = String::new();
    let json = serde_json::to_string(&clone).expect("report serializes");
    let hash = Sha256::digest(json.as_bytes());
    let mut binder = Vec::with_capacity(hash.len() + SCHOLARLY_EVIDENCE_SCHEMA_VERSION.len());
    binder.extend_from_slice(&hash);
    binder.extend_from_slice(SCHOLARLY_EVIDENCE_SCHEMA_VERSION.as_bytes());
    hex::encode(Sha256::digest(&binder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> ContentDigest {
        ContentDigest::of_text(seed)
    }

    fn sample_input() -> ScholarlyEvidenceInput {
        ScholarlyEvidenceInput {
            article_id: "2606.06036".into(),
            title: Some("Memory is Reconstructed, Not Retrieved".into()),
            providers: vec![
                ProviderObservation {
                    provider: "openalex".into(),
                    status: "fetched".into(),
                    payload_sha256: Some(digest("oa")),
                    work_id: Some("https://openalex.org/W1".into()),
                    doi: Some("10.48550/arxiv.2606.06036".into()),
                    title: Some("Memory is Reconstructed, Not Retrieved".into()),
                    authors: vec!["A".into()],
                    cited_by_count: Some(0),
                    referenced_works_count: Some(0),
                    topics: vec!["Graph".into()],
                },
                ProviderObservation {
                    provider: "semantic_scholar".into(),
                    status: "rate_limited".into(),
                    payload_sha256: None,
                    work_id: None,
                    doi: None,
                    title: None,
                    authors: vec![],
                    cited_by_count: Some(2),
                    referenced_works_count: Some(24),
                    topics: vec![],
                },
            ],
            tex: Some(LocalTexObservation {
                analysis_sha256: digest("tex"),
                archive_sha256: digest("arc"),
                root_document: Some("main.tex".into()),
                status: "conflict".into(),
                local_reference_denominator: Some(30),
                bibliography: vec![
                    LocalBibObservation {
                        candidate_id: digest("b1"),
                        entry_key: "k1".into(),
                        entry_type: "article".into(),
                        title: Some("Other paper".into()),
                        doi: Some("10.1/abc".into()),
                        eprint: None,
                        source_member: "refs.bib".into(),
                    },
                    LocalBibObservation {
                        candidate_id: digest("b2"),
                        entry_key: "k2".into(),
                        entry_type: "misc".into(),
                        title: Some("No id".into()),
                        doi: None,
                        eprint: None,
                        source_member: "refs.bib".into(),
                    },
                ],
                diagnostics: vec!["provider_denominator_conflict:openalex:0:30".into()],
            }),
            import_eligible: false,
        }
    }

    #[test]
    fn reconcile_surfaces_denominator_and_rate_limit_conflicts() {
        let report = reconcile_scholarly_evidence(sample_input()).unwrap();
        assert_eq!(report.verdict, ScholarlyReconcileVerdict::NeedsReview);
        assert!(!report.import_eligible);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.class == "citation_denominator_conflict")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.class == "provider_rate_limited")
        );
        assert_eq!(report.local_reference_denominator, Some(30));
        assert_eq!(report.unmatched_local_bibliography, 1);
        assert!(report.validate().is_ok());
    }

    #[test]
    fn d127_unlock_rejected() {
        let mut input = sample_input();
        let unlock = !input.import_eligible;
        input.import_eligible = unlock;
        assert!(reconcile_scholarly_evidence(input).is_err());
    }

    #[test]
    fn deterministic_digest() {
        let a = reconcile_scholarly_evidence(sample_input()).unwrap();
        let b = reconcile_scholarly_evidence(sample_input()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_roundtrip() {
        let report = reconcile_scholarly_evidence(sample_input()).unwrap();
        let json = serde_json::to_vec(&report).unwrap();
        let back: ScholarlyEvidenceReport = serde_json::from_slice(&json).unwrap();
        assert_eq!(report, back);
        assert!(back.validate().is_ok());
    }
}
