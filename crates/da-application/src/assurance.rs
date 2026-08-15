//! ACAP application use cases (ADR-055).
//!
//! The checker consumes explicit observations from a materialization boundary.
//! It never writes graph truth and depends only on domain values and ports.

use crate::ingest::IngestResult;
use da_domain::{
    ArticleAssuranceReport, ArticleContextState, BoundaryMode, ConservationObservation,
    ContentDigest, CountObservation, LayerAssuranceResult, MissingnessReason,
    MutationFamilyObservation, ReplayObservation, WriteDelta,
};
use da_ports::parser::ParsedArticle;
use da_ports::{AssuranceReportStore, DirectGraphStore, MutationCounters};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedSectionObservation {
    /// Source parser ordinal. It is evidence identity, not the long-term graph VID contract.
    pub ordinal: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationReadback {
    pub abstract_text: Option<String>,
    pub sections: Vec<MaterializedSectionObservation>,
    pub paper_section_count: Option<u64>,
    pub paper_citation_count: Option<u64>,
    pub import_eligible_true_nodes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MaterializationReplayObservation {
    #[default]
    NotRun,
    /// Full physical partition. Authoritative for ACAP hard gates.
    Observed(ReplayObservation),
}

impl MaterializationReplayObservation {
    /// Map kg-storage MutationCounters one-to-one into domain ReplayObservation.
    ///
    /// Invalid / empty / unobserved counters are still preserved as Observed so
    /// the report can diagnose *why* integrity fails. Only a total absence of
    /// instrumentation yields NotRun.
    pub fn from_mutations(counters: MutationCounters) -> Self {
        // Completely empty counters with zero unobserved ops = instrumentation
        // never ran. Everything else is an observation that must be evaluated.
        if counters == MutationCounters::default() {
            return Self::NotRun;
        }
        Self::Observed(ReplayObservation {
            node: family(counters.node),
            property: family(counters.property),
            edge: family(counters.edge),
            vector: family(counters.vector),
            unobserved_operations: counters.unobserved_operations,
        })
    }

    /// Diagnostic WriteDelta projection when present.
    pub fn write_delta(self) -> Option<WriteDelta> {
        match self {
            Self::NotRun => None,
            Self::Observed(obs) => Some(obs.applied_write_delta()),
        }
    }

    pub fn replay_observation(self) -> Option<ReplayObservation> {
        match self {
            Self::NotRun => None,
            Self::Observed(obs) => Some(obs),
        }
    }
}

fn family(c: da_ports::MutationFamilyCounters) -> MutationFamilyObservation {
    MutationFamilyObservation {
        attempted: c.attempted,
        applied: c.applied,
        unchanged: c.unchanged,
        rejected: c.rejected,
    }
}

pub struct MaterializationAssuranceUseCase<'a> {
    store: Option<&'a dyn AssuranceReportStore>,
}

impl<'a> MaterializationAssuranceUseCase<'a> {
    pub fn new() -> Self {
        Self { store: None }
    }

    pub fn with_store(store: &'a dyn AssuranceReportStore) -> Self {
        Self { store: Some(store) }
    }

    /// Read the canonical Paper/Section sink and evaluate it against parser
    /// output. Section lookup follows the current materializer identity contract;
    /// stable section identity migration is deliberately a separate slice.
    pub async fn evaluate_graph(
        &self,
        graph: &dyn DirectGraphStore,
        parsed: &ParsedArticle,
        ingest: &IngestResult,
        replay: MaterializationReplayObservation,
        policy_sha256: ContentDigest,
    ) -> anyhow::Result<ArticleAssuranceReport> {
        let d127_unlock_count = Self::count_d127_unlocks(graph).await;
        self.evaluate_graph_with_d127(
            graph,
            parsed,
            ingest,
            replay,
            policy_sha256,
            d127_unlock_count,
        )
        .await
    }

    pub async fn count_d127_unlocks(graph: &dyn DirectGraphStore) -> u64 {
        let mut count = 0_u64;
        for node in graph.get_all_node_ids().await {
            count += u64::from(
                graph.get_node_property_bool(node, "import_eligible").await == Some(true),
            );
        }
        count
    }

    /// Evaluate one article using a corpus-scoped D127 count already measured
    /// after the ordered commit phase. Batch orchestrators use this to avoid an
    /// O(articles × graph_nodes) full-graph scan per article.
    pub async fn evaluate_graph_with_d127(
        &self,
        graph: &dyn DirectGraphStore,
        parsed: &ParsedArticle,
        ingest: &IngestResult,
        replay: MaterializationReplayObservation,
        policy_sha256: ContentDigest,
        d127_unlock_count: u64,
    ) -> anyhow::Result<ArticleAssuranceReport> {
        let paper_vid = da_domain::paper_vid(&parsed.paper_id);
        let paper_node = graph
            .find_node_by_string_property("Paper", "vid", &paper_vid)
            .await;
        let mut readback = MaterializationReadback {
            abstract_text: None,
            sections: Vec::with_capacity(parsed.sections.len()),
            paper_section_count: None,
            paper_citation_count: None,
            import_eligible_true_nodes: d127_unlock_count,
        };
        if let Some(node) = paper_node {
            readback.abstract_text = graph.get_node_property_string(node, "abstract_text").await;
            readback.paper_section_count = graph
                .get_node_property_int(node, "section_count")
                .await
                .and_then(|value| u64::try_from(value).ok());
            readback.paper_citation_count = graph
                .get_node_property_int(node, "citation_count")
                .await
                .and_then(|value| u64::try_from(value).ok());
        }
        for ordinal in 0..parsed.sections.len() {
            let section_vid = format!("vid:section:{}:{}", parsed.paper_id, ordinal);
            if let Some(node) = graph
                .find_node_by_string_property("Section", "vid", &section_vid)
                .await
                && let Some(text) = graph.get_node_property_string(node, "text").await
            {
                readback
                    .sections
                    .push(MaterializedSectionObservation { ordinal, text });
            }
        }
        self.evaluate(parsed, ingest, &readback, replay, policy_sha256)
    }

    pub fn evaluate(
        &self,
        parsed: &ParsedArticle,
        ingest: &IngestResult,
        readback: &MaterializationReadback,
        replay: MaterializationReplayObservation,
        policy_sha256: ContentDigest,
    ) -> anyhow::Result<ArticleAssuranceReport> {
        let mut conservations = Vec::with_capacity(parsed.sections.len() + 1);
        let mut available_required_fields = 0_u64;
        let mut retained_required_fields = 0_u64;

        available_required_fields += 1;
        let abstract_sink = readback.abstract_text.as_deref().unwrap_or_default();
        if readback.abstract_text.is_some() {
            retained_required_fields += 1;
        }
        conservations.push(conservation(
            "paper.abstract_text",
            &parsed.abstract_text,
            abstract_sink,
            readback
                .abstract_text
                .is_none()
                .then_some(MissingnessReason::NotMaterialized),
        ));

        let sections_by_ordinal: BTreeMap<usize, &str> = readback
            .sections
            .iter()
            .map(|section| (section.ordinal, section.text.as_str()))
            .collect();
        for (ordinal, source) in parsed.sections.iter().enumerate() {
            available_required_fields += 1;
            let sink = sections_by_ordinal
                .get(&ordinal)
                .copied()
                .unwrap_or_default();
            if sections_by_ordinal.contains_key(&ordinal) {
                retained_required_fields += 1;
            }
            conservations.push(conservation(
                format!("section[{ordinal}].text"),
                &source.text,
                sink,
                (!sections_by_ordinal.contains_key(&ordinal))
                    .then_some(MissingnessReason::NotMaterialized),
            ));
        }

        let observed_sections = readback.paper_section_count.unwrap_or(0);
        let observed_citations = readback.paper_citation_count.unwrap_or(0);
        let mut layer = LayerAssuranceResult {
            layer_id: "materialization".into(),
            state: ArticleContextState::Materialized,
            article_count: 1,
            available_required_fields,
            retained_required_fields,
            conservations,
            counts: vec![
                CountObservation {
                    name: "paper.section_count".into(),
                    expected: parsed.sections.len() as u64,
                    observed: observed_sections,
                },
                CountObservation {
                    name: "paper.citation_count".into(),
                    expected: parsed.citations.len() as u64,
                    observed: observed_citations,
                },
                CountObservation {
                    name: "ingest.section_count".into(),
                    expected: parsed.sections.len() as u64,
                    observed: ingest.section_count as u64,
                },
                CountObservation {
                    name: "ingest.citation_count".into(),
                    expected: parsed.citations.len() as u64,
                    observed: ingest.citation_count as u64,
                },
                CountObservation {
                    name: "nonempty_body_has_structural_sections".into(),
                    expected: u64::from(!parsed.body_text.is_empty()),
                    observed: u64::from(
                        !parsed.body_text.is_empty() && !parsed.sections.is_empty(),
                    ),
                },
            ],
            replay_writes: replay.write_delta(),
            replay_observation: replay.replay_observation(),
            // Replay is a required hard gate at the materialization layer.
            // NotApplicable is reserved for pre-materialization layers; marking
            // NotRun as NotApplicable here would silently turn missing physical
            // write evidence into PASS.
            replay_missingness: None,
            d127_unlock_count: readback.import_eligible_true_nodes
                + u64::from(ingest.import_eligible),
            quality_metrics: BTreeMap::from([(
                "coverage".into(),
                retained_required_fields as f64 / available_required_fields as f64,
            )]),
            hard_failures: vec![],
        };
        layer.evaluate_hard_failures();
        let report = ArticleAssuranceReport::new(
            parsed.paper_id.clone(),
            policy_sha256,
            ArticleContextState::Materialized,
            vec![layer],
        )?;
        if let Some(store) = self.store {
            store.store(&report)?;
        }
        Ok(report)
    }
}

impl Default for MaterializationAssuranceUseCase<'_> {
    fn default() -> Self {
        Self::new()
    }
}

fn conservation(
    field: impl Into<String>,
    source: &str,
    sink: &str,
    missingness: Option<MissingnessReason>,
) -> ConservationObservation {
    ConservationObservation {
        field: field.into(),
        mode: BoundaryMode::FullFidelity,
        source_bytes: source.len() as u64,
        sink_bytes: sink.len() as u64,
        source_sha256: ContentDigest::of_text(source),
        sink_sha256: ContentDigest::of_text(sink),
        missingness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{AssuranceVerdict, PdfClassification};
    use da_ports::parser::{CitationEntry, Section};

    fn parsed() -> ParsedArticle {
        ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "ABSTRACT-尾".into(),
            body_text: "Intro TAIL-科学".into(),
            sections: vec![Section {
                title: "Intro".into(),
                text: "Intro TAIL-科学".into(),
                level: 1,
                section_id: Some("sec-intro".into()),
                ..Default::default()
            }],
            citations: vec![CitationEntry {
                raw_text: "Reference".into(),
                doi: None,
                arxiv_id: None,
                title: Some("Reference".into()),
                anchors: vec![],
            }],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "source-sha".into(),
        }
    }

    fn ingest() -> IngestResult {
        IngestResult {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            body_chars: "Intro TAIL-科学".chars().count(),
            section_count: 1,
            sections_written: 1,
            citation_count: 1,
            references_written: 1,
            cites_resolved: 0,
            vector_dimensions: 3,
            graph_node_id: Some(1),
            revision_vid: None,
            import_eligible: false,
            pdf_classification: None::<PdfClassification>,
            pdf_triage: da_domain::PdfTriageStatus::NotConfigured,
        }
    }

    fn readback() -> MaterializationReadback {
        MaterializationReadback {
            abstract_text: Some("ABSTRACT-尾".into()),
            sections: vec![MaterializedSectionObservation {
                ordinal: 0,
                text: "Intro TAIL-科学".into(),
            }],
            paper_section_count: Some(1),
            paper_citation_count: Some(1),
            import_eligible_true_nodes: 0,
        }
    }

    #[test]
    fn full_unicode_materialization_passes_conservation() {
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                MaterializationReplayObservation::Observed(
                    ReplayObservation::valid_unchanged_property(1),
                ),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Pass);
        assert!(report.layer_results[0].hard_failures.is_empty());
    }

    #[test]
    fn truncated_section_and_count_drift_fail_closed() {
        let mut sink = readback();
        sink.sections[0].text = "Intro".into();
        sink.paper_section_count = Some(0);
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &sink,
                MaterializationReplayObservation::Observed(
                    ReplayObservation::valid_unchanged_property(1),
                ),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .iter()
                .any(|item| item == "byte_conservation:section[0].text")
        );
        assert!(
            report.layer_results[0]
                .hard_failures
                .iter()
                .any(|item| item.starts_with("count:paper.section_count"))
        );
    }

    #[test]
    fn pure_rejection_retains_specific_failure_diagnostic() {
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                MaterializationReplayObservation::from_mutations(MutationCounters {
                    property: da_ports::MutationFamilyCounters {
                        attempted: 1,
                        applied: 0,
                        unchanged: 0,
                        rejected: 1,
                    },
                    ..MutationCounters::default()
                }),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"replay_rejected_writes:1".into())
        );
    }

    #[test]
    fn property_mutation_from_instrumented_store_fails_replay() {
        let observation = MaterializationReplayObservation::from_mutations(MutationCounters {
            property: da_ports::MutationFamilyCounters {
                attempted: 2,
                applied: 1,
                unchanged: 1,
                rejected: 0,
            },
            ..MutationCounters::default()
        });
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                observation,
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"replay_net_writes:1".into())
        );
    }

    #[test]
    fn empty_mutation_counters_are_missing_evidence() {
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                MaterializationReplayObservation::from_mutations(MutationCounters::default()),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"replay_net_writes:missing_observation".into())
        );
    }

    #[test]
    fn invalid_partition_counters_are_preserved_and_fail_integrity() {
        let counters = MutationCounters {
            property: da_ports::MutationFamilyCounters {
                attempted: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                MaterializationReplayObservation::from_mutations(counters),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        let failures = &report.layer_results[0].hard_failures;
        assert!(
            failures.contains(&"replay_observation_integrity:invalid_partition".into()),
            "failures: {failures:?}"
        );
        // Full observation is preserved in the report for independent recomputation.
        let obs = report.layer_results[0].replay_observation.unwrap();
        assert_eq!(obs.property.attempted, 1);
        assert!(!obs.integrity_passes());
    }

    #[test]
    fn missing_replay_observation_fails_closed() {
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &readback(),
                MaterializationReplayObservation::NotRun,
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"replay_net_writes:missing_observation".into())
        );
    }

    #[test]
    fn replay_writes_and_d127_unlock_fail_even_with_full_fidelity() {
        let mut sink = readback();
        sink.import_eligible_true_nodes = 1;
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &sink,
                MaterializationReplayObservation::Observed(ReplayObservation {
                    node: MutationFamilyObservation {
                        attempted: 1,
                        applied: 1,
                        unchanged: 0,
                        rejected: 0,
                    },
                    edge: MutationFamilyObservation {
                        attempted: 2,
                        applied: 2,
                        unchanged: 0,
                        rejected: 0,
                    },
                    ..ReplayObservation::default()
                }),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"replay_net_writes:3".into())
        );
        assert!(
            report.layer_results[0]
                .hard_failures
                .contains(&"d127_unlock_count:1".into())
        );
    }

    #[test]
    fn nonempty_body_without_sections_fails_closed() {
        let mut source = parsed();
        source.sections.clear();
        let mut sink = readback();
        sink.sections.clear();
        sink.paper_section_count = Some(0);
        let mut result = ingest();
        result.section_count = 0;
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &source,
                &result,
                &sink,
                MaterializationReplayObservation::Observed(
                    ReplayObservation::valid_unchanged_property(1),
                ),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        assert!(
            report.layer_results[0].hard_failures.iter().any(|failure| {
                failure.starts_with("count:nonempty_body_has_structural_sections")
            })
        );
    }

    #[test]
    fn absent_section_is_typed_and_not_zero_over_zero() {
        let mut sink = readback();
        sink.sections.clear();
        let report = MaterializationAssuranceUseCase::new()
            .evaluate(
                &parsed(),
                &ingest(),
                &sink,
                MaterializationReplayObservation::Observed(
                    ReplayObservation::valid_unchanged_property(1),
                ),
                ContentDigest::of_text("policy"),
            )
            .unwrap();
        let layer = &report.layer_results[0];
        assert_eq!(layer.available_required_fields, 2);
        assert_eq!(layer.retained_required_fields, 1);
        assert_eq!(
            layer.conservations[1].missingness,
            Some(MissingnessReason::NotMaterialized)
        );
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
    }
}
