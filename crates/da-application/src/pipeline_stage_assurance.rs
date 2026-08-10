//! Project required pre-materialization ACAP layers from durable wave evidence
//! (M371 post-S07 thin product wave).
//!
//! Pure application composition: no filesystem, no graph I/O. Callers inject
//! checksum lineage and first-pass graph counters already proven by the
//! materialization artifact. Layers that lack physical graph replay set
//! `replay_missingness=NotApplicable` so hard gates stay typed.

use std::collections::BTreeMap;

use da_domain::{
    ArticleAssuranceReport, ArticleContextState, ContentDigest, CountObservation,
    LayerAssuranceResult, MissingnessReason, MutationFamilyObservation, ReplayObservation,
};
use serde::{Deserialize, Serialize};

/// Checksum-bound stage artifacts from the cache-to-graph wave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageArtifactLineage {
    pub source_sha256: ContentDigest,
    pub parse_sha256: ContentDigest,
    pub extract_sha256: ContentDigest,
    pub embed_sha256: ContentDigest,
}

/// First-pass graph object counts retained as diagnostics (not NetWrites proof).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageGraphCounts {
    pub sections: u64,
    pub references: u64,
    pub citations: u64,
    pub entities: u64,
    pub mentions: u64,
    pub found_in: u64,
    pub evidence_bundles: u64,
    pub claims: u64,
}

/// Physical replay partition for the materialization layer only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageMaterializationReplay {
    pub observation: ReplayObservation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStageAssuranceInput {
    pub article_id: String,
    pub policy_sha256: ContentDigest,
    pub lineage: StageArtifactLineage,
    pub first: StageGraphCounts,
    /// Optional materialization replay; when present it is authoritative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialization_replay: Option<StageMaterializationReplay>,
    pub d127_unlock_count: u64,
    pub import_eligible: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineStageAssuranceError {
    #[error(transparent)]
    Domain(#[from] da_domain::AssuranceError),
    #[error("invalid pipeline stage assurance input: {0}")]
    InvalidInput(String),
}

pub struct PipelineStageAssuranceUseCase;

impl PipelineStageAssuranceUseCase {
    /// Build a multi-layer per-article report from durable wave evidence.
    ///
    /// Layer set: source, parse, structure, extraction, evidence, embedding,
    /// materialization (when replay is supplied). Missing counts fail closed via
    /// CountObservation expected/observed inequality — never invented PASS.
    pub fn evaluate(
        input: PipelineStageAssuranceInput,
    ) -> Result<ArticleAssuranceReport, PipelineStageAssuranceError> {
        validate_input(&input)?;

        let d127 = if input.import_eligible {
            input.d127_unlock_count.max(1)
        } else {
            input.d127_unlock_count
        };

        let mut layers = vec![
            source_layer(d127),
            parse_layer(d127),
            structure_layer(&input, d127),
            extraction_layer(&input, d127),
            evidence_layer(&input, d127),
            embedding_layer(d127),
        ];
        if let Some(replay) = input.materialization_replay {
            layers.push(materialization_layer(&input, d127, replay.observation));
        }

        let current_state = layers
            .last()
            .map(|layer| layer.state)
            .unwrap_or(ArticleContextState::SourceRegistered);

        Ok(ArticleAssuranceReport::new(
            input.article_id,
            input.policy_sha256,
            current_state,
            layers,
        )?)
    }
}

fn validate_input(input: &PipelineStageAssuranceInput) -> Result<(), PipelineStageAssuranceError> {
    if input.article_id.trim().is_empty() {
        return Err(PipelineStageAssuranceError::InvalidInput(
            "article_id must not be empty".into(),
        ));
    }
    if input.import_eligible && input.d127_unlock_count == 0 {
        return Err(PipelineStageAssuranceError::InvalidInput(
            "import_eligible true requires d127_unlock_count >= 1".into(),
        ));
    }
    Ok(())
}

fn pre_materialization_base(
    layer_id: &str,
    state: ArticleContextState,
    available: u64,
    retained: u64,
    d127: u64,
    counts: Vec<CountObservation>,
    quality: BTreeMap<String, f64>,
) -> LayerAssuranceResult {
    LayerAssuranceResult {
        layer_id: layer_id.into(),
        state,
        article_count: 1,
        available_required_fields: available,
        retained_required_fields: retained,
        conservations: vec![],
        counts,
        replay_writes: None,
        replay_observation: None,
        replay_missingness: Some(MissingnessReason::NotApplicable),
        d127_unlock_count: d127,
        quality_metrics: quality,
        hard_failures: vec![],
    }
}

fn source_layer(d127: u64) -> LayerAssuranceResult {
    // Required fields: paper_id + source_sha256 lineage (validated digests).
    pre_materialization_base(
        "source",
        ArticleContextState::SourceVerified,
        2,
        2,
        d127,
        vec![],
        BTreeMap::from([("coverage".into(), 1.0)]),
    )
}

fn parse_layer(d127: u64) -> LayerAssuranceResult {
    // Required: source lineage + parse artifact checksum.
    pre_materialization_base(
        "parse",
        ArticleContextState::Parsed,
        2,
        2,
        d127,
        vec![],
        BTreeMap::from([("coverage".into(), 1.0)]),
    )
}

fn structure_layer(input: &PipelineStageAssuranceInput, d127: u64) -> LayerAssuranceResult {
    let available = 1_u64;
    let retained = u64::from(input.first.sections > 0);
    let counts = vec![CountObservation {
        name: "sections".into(),
        expected: 1,
        observed: input.first.sections.min(1),
    }];
    pre_materialization_base(
        "structure",
        ArticleContextState::Structured,
        available,
        retained,
        d127,
        counts,
        BTreeMap::from([(
            "coverage".into(),
            if input.first.sections > 0 { 1.0 } else { 0.0 },
        )]),
    )
}

fn extraction_layer(input: &PipelineStageAssuranceInput, d127: u64) -> LayerAssuranceResult {
    // Required: extract artifact checksum + at least one mention.
    // Entity count is diagnostic (expected == observed always).
    let available = 2_u64;
    let has_mentions = input.first.mentions > 0;
    let retained = 1 + u64::from(has_mentions);
    let counts = vec![
        CountObservation {
            name: "entities".into(),
            expected: input.first.entities,
            observed: input.first.entities,
        },
        CountObservation {
            name: "mentions".into(),
            expected: 1,
            observed: input.first.mentions.min(1),
        },
    ];
    pre_materialization_base(
        "extraction",
        ArticleContextState::Extracted,
        available,
        retained,
        d127,
        counts,
        BTreeMap::from([("coverage".into(), if has_mentions { 1.0 } else { 0.0 })]),
    )
}

fn evidence_layer(input: &PipelineStageAssuranceInput, d127: u64) -> LayerAssuranceResult {
    // FOUND_IN is required when mentions exist (section grounding).
    // EvidenceBundle is required only when Claims exist — the current materializer
    // does not invent bundles for mention-only papers (typed zero expectation).
    let no_mentions = input.first.mentions == 0;
    let has_claims = input.first.claims > 0;
    let has_found_in = input.first.found_in > 0;
    let has_bundles = input.first.evidence_bundles > 0;

    let found_in_count = CountObservation {
        name: "found_in".into(),
        expected: if no_mentions { 0 } else { 1 },
        observed: if no_mentions {
            0
        } else {
            input.first.found_in.min(1)
        },
    };
    let bundles_count = CountObservation {
        name: "evidence_bundles".into(),
        expected: if has_claims { 1 } else { 0 },
        observed: if has_claims {
            input.first.evidence_bundles.min(1)
        } else {
            0
        },
    };

    // available/retained must stay equal on success (domain hard gate).
    let found_in_ok = no_mentions || has_found_in;
    let bundles_ok = !has_claims || has_bundles;
    let available = 1_u64 + u64::from(has_claims);
    let retained = u64::from(found_in_ok) + u64::from(has_claims && bundles_ok);
    // When claims are absent, bundles dimension is not in the required set.
    let retained = if has_claims {
        retained
    } else {
        u64::from(found_in_ok)
    };
    let coverage = if available == 0 {
        1.0
    } else {
        retained as f64 / available as f64
    };

    pre_materialization_base(
        "evidence",
        ArticleContextState::EvidenceLinked,
        available,
        retained,
        d127,
        vec![found_in_count, bundles_count],
        BTreeMap::from([("coverage".into(), coverage)]),
    )
}

fn embedding_layer(d127: u64) -> LayerAssuranceResult {
    pre_materialization_base(
        "embedding",
        ArticleContextState::Embedded,
        1,
        1,
        d127,
        vec![],
        BTreeMap::from([("coverage".into(), 1.0)]),
    )
}

fn materialization_layer(
    input: &PipelineStageAssuranceInput,
    d127: u64,
    observation: ReplayObservation,
) -> LayerAssuranceResult {
    let available = 1_u64 + u64::from(input.first.sections > 0);
    let retained = available;
    LayerAssuranceResult {
        layer_id: "materialization".into(),
        state: ArticleContextState::Materialized,
        article_count: 1,
        available_required_fields: available,
        retained_required_fields: retained,
        conservations: vec![],
        counts: vec![CountObservation {
            name: "sections".into(),
            expected: input.first.sections,
            observed: input.first.sections,
        }],
        replay_writes: Some(observation.applied_write_delta()),
        replay_observation: Some(observation),
        replay_missingness: None,
        d127_unlock_count: d127,
        quality_metrics: BTreeMap::from([("coverage".into(), 1.0)]),
        hard_failures: vec![],
    }
}

/// Rebuild ReplayObservation from paper-level mutation counter family tuples
/// `(attempted, applied, unchanged, rejected)`.
pub fn replay_from_family_counts(
    node: (u64, u64, u64, u64),
    property: (u64, u64, u64, u64),
    edge: (u64, u64, u64, u64),
    vector: (u64, u64, u64, u64),
    unobserved_operations: u64,
) -> ReplayObservation {
    fn fam(t: (u64, u64, u64, u64)) -> MutationFamilyObservation {
        MutationFamilyObservation {
            attempted: t.0,
            applied: t.1,
            unchanged: t.2,
            rejected: t.3,
        }
    }
    ReplayObservation {
        node: fam(node),
        property: fam(property),
        edge: fam(edge),
        vector: fam(vector),
        unobserved_operations,
    }
}

/// Parse a hex digest string into ContentDigest (fail-closed).
pub fn digest_hex(value: &str) -> Result<ContentDigest, PipelineStageAssuranceError> {
    ContentDigest::try_from(value.to_string()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AssuranceVerdict;

    fn digest(seed: &str) -> ContentDigest {
        ContentDigest::of_text(seed)
    }

    fn valid_input() -> PipelineStageAssuranceInput {
        PipelineStageAssuranceInput {
            article_id: "2507.19457".into(),
            policy_sha256: digest("policy"),
            lineage: StageArtifactLineage {
                source_sha256: digest("source"),
                parse_sha256: digest("parse"),
                extract_sha256: digest("extract"),
                embed_sha256: digest("embed"),
            },
            first: StageGraphCounts {
                sections: 10,
                references: 5,
                citations: 3,
                entities: 4,
                mentions: 4,
                found_in: 4,
                evidence_bundles: 1,
                claims: 1,
            },
            materialization_replay: Some(StageMaterializationReplay {
                observation: ReplayObservation::valid_unchanged_property(12),
            }),
            d127_unlock_count: 0,
            import_eligible: false,
        }
    }

    #[test]
    fn projects_all_required_layers_with_materialization() {
        let report = PipelineStageAssuranceUseCase::evaluate(valid_input()).unwrap();
        let ids: Vec<_> = report
            .layer_results
            .iter()
            .map(|layer| layer.layer_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "source",
                "parse",
                "structure",
                "extraction",
                "evidence",
                "embedding",
                "materialization",
            ]
        );
        assert_eq!(report.verdict, AssuranceVerdict::Pass);
        assert!(
            report
                .layer_results
                .iter()
                .all(|layer| layer.d127_unlock_count == 0)
        );
    }

    #[test]
    fn zero_entities_and_mentions_types_evidence_zero_expectations() {
        let mut input = valid_input();
        input.first.entities = 0;
        input.first.mentions = 0;
        input.first.found_in = 0;
        input.first.evidence_bundles = 0;
        input.first.claims = 0;
        let report = PipelineStageAssuranceUseCase::evaluate(input).unwrap();
        // extraction fails on missing mentions
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        let extraction = report
            .layer_results
            .iter()
            .find(|layer| layer.layer_id == "extraction")
            .unwrap();
        assert!(!extraction.hard_failures.is_empty());
        let evidence = report
            .layer_results
            .iter()
            .find(|layer| layer.layer_id == "evidence")
            .unwrap();
        assert!(
            evidence.hard_failures.is_empty(),
            "evidence: {:?}",
            evidence.hard_failures
        );
    }

    #[test]
    fn mention_only_papers_require_found_in_not_evidence_bundles() {
        let mut input = valid_input();
        input.first.entities = 0;
        input.first.mentions = 3;
        input.first.found_in = 3;
        input.first.evidence_bundles = 0;
        input.first.claims = 0;
        let report = PipelineStageAssuranceUseCase::evaluate(input).unwrap();
        let evidence = report
            .layer_results
            .iter()
            .find(|layer| layer.layer_id == "evidence")
            .unwrap();
        assert!(
            evidence.hard_failures.is_empty(),
            "evidence: {:?}",
            evidence.hard_failures
        );
        assert_eq!(report.verdict, AssuranceVerdict::Pass);
    }

    #[test]
    fn claims_require_evidence_bundles() {
        let mut input = valid_input();
        input.first.claims = 2;
        input.first.evidence_bundles = 0;
        let report = PipelineStageAssuranceUseCase::evaluate(input).unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        let evidence = report
            .layer_results
            .iter()
            .find(|layer| layer.layer_id == "evidence")
            .unwrap();
        assert!(
            evidence
                .hard_failures
                .iter()
                .any(|f| f.contains("evidence_bundles"))
        );
    }

    #[test]
    fn missing_sections_fails_structure() {
        let mut input = valid_input();
        input.first.sections = 0;
        let report = PipelineStageAssuranceUseCase::evaluate(input).unwrap();
        assert_eq!(report.verdict, AssuranceVerdict::Fail);
        let structure = report
            .layer_results
            .iter()
            .find(|layer| layer.layer_id == "structure")
            .unwrap();
        assert!(!structure.hard_failures.is_empty());
    }

    #[test]
    fn pre_materialization_layers_do_not_require_physical_replay() {
        let mut input = valid_input();
        input.materialization_replay = None;
        let report = PipelineStageAssuranceUseCase::evaluate(input).unwrap();
        assert_eq!(report.layer_results.len(), 6);
        for layer in &report.layer_results {
            assert!(
                !layer.hard_failures.iter().any(|f| f.contains("replay")),
                "{}: {:?}",
                layer.layer_id,
                layer.hard_failures
            );
        }
        // Without materialization, remaining layers can still pass.
        assert_eq!(report.verdict, AssuranceVerdict::Pass);
    }
}
