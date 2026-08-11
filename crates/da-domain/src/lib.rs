//! da-domain — pure domain types for daily-archive v2.
//!
//! Zero infrastructure dependencies. Only serde + chrono + sha2.
//! This is the single source of truth for all data shapes.
//!
//! ADR-037 §2: Domain layer depends on nothing (only std + serde).
//! ADR-038 §2: 5-module schema (A-E), 18 relation types.
//! ADR-040 §11: Schema enforcement lives here, not in DDL.

pub mod article;
pub mod assurance;
pub mod catalog;
pub mod cluster;
pub mod corpus_checkpoint;
pub mod domain;
pub mod edge_contract;
pub mod entity;
pub mod entity_resolution;
pub mod eval;
pub mod evidence;
pub mod evidence_bundle;
pub mod extractive_summary;
pub mod figure;
pub mod figure_analysis;
pub mod figure_context;
pub mod finalize;
pub mod graph_tasks;
pub mod healing;
pub mod hypergraph;
pub mod load;
pub mod materialization_profile;
pub mod ordered_mutation;
pub mod paper;
pub mod process;
pub mod relation;
pub mod revision_readiness;
pub mod scheduler;
pub mod schema;
pub mod scholarly_evidence;
pub mod section_summary_policy;
pub mod source;
pub mod temporal;
pub mod tex_source;
pub mod validator;
pub mod vid;
pub mod vocabulary;
pub mod yaml_schema_adapter;

pub use article::{
    Author, AuthorSchema, Category, CategorySchema, Concept, ConceptSchema, Institution,
    InstitutionSchema, Reference, ReferenceSchema, Section, SectionSchema, Topic, TopicSchema,
};
pub use assurance::{
    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ACAP_POLICY_ID, ACAP_REPORT_SCHEMA_VERSION,
    ArticleAssuranceReport, ArticleContextState, AssuranceError, AssuranceVerdict, BoundaryMode,
    ConservationObservation, ContentDigest, CorpusAggregateMetrics, CorpusAssuranceReport,
    CorpusLayerResult, CorpusLayerStatus, CountObservation, LayerAssuranceResult,
    MissingnessReason, MutationFamilyObservation, ReplayObservation, WriteDelta,
    corpus_report_digest,
};
pub use edge_contract::{
    EdgeContract, contract_edge_types, contract_edge_types_or_yaml, edge_contracts,
    edge_contracts_or_yaml, render_edge_contracts_yaml, render_markdown_table,
    validate_compiled_edge_endpoint,
};
pub use entity::{ENTITY_TYPES, Entity, EntitySchema, EntityType};
pub use eval::{
    ExtractionMetrics, GoldEntity, MultiPaperGoldIndex, PredictedEntity, list_false_positives,
    promote_candidates,
};
pub use evidence::{
    EpistemicStatus, EvidenceAssertion, EvidenceId, EvidenceVerdict, EvidenceVerification,
    SourceSpan, SpanType,
};
pub use figure::{
    BoundingBox72Dpi, DetectedVisualArtifact, VisualArtifactError, VisualArtifactKind,
};
pub use figure_analysis::{
    FigureAnalysis, FigureAnalysisCitation, FigureAnalysisComponent, FigureAnalysisError,
    FigureAnalysisProvenance, FigureAnalysisRelationship, VisualAnalysisType,
};
pub use figure_context::{
    ArticleSummaryContext, FigureContextError, FigureReferenceExcerpt, FigureSectionContext,
    FigureSectionRelation, FigureSemanticContext,
};
pub use healing::{
    CorrectResult, HealingActor, HealingOperation, MergeResult, ProvenanceEvent, SilenceResult,
};
pub use hypergraph::ConceptClusterSchema;
pub use paper::{Paper, PaperSchema, PaperStatus};
pub use relation::hypergraph::{
    CONTRADICTS, MEMBER_OF_CLUSTER, PARTICIPATES_IN, QUALIFIES, SUPPORTS,
};
pub use tex_source::{
    BibtexCandidate, ProviderDenominator, TEX_ANALYSIS_SCHEMA_VERSION, TexAnalysisError,
    TexAnalysisPolicy, TexAnalysisStatus, TexIncludeEdge, TexSourceAnalysisReport,
    analyze_tex_members,
};
pub mod confidence;
pub use confidence::{clamp_confidence, is_unit_interval};

pub mod import_eligibility;
pub use import_eligibility::ImportEligibility;

pub mod pdf_classification;
pub use load::{
    AcquisitionPolicy, ArticleLocator, AssetDisposition, LOAD_REPORT_SCHEMA_VERSION,
    LoadAssetRecord, LoadCacheRecord, LoadDomainError, LoadFinding, LoadQueueRecord,
    LoadQueueTaskRef, LoadReport, LoadRunMode, LoadStageName, LoadStageSet, PrimaryContentKind,
    StageStatus, try_normalize_arxiv_id, try_parse_arxiv_locator,
};
pub use pdf_classification::{
    OCR_REASON_NO_TEXT, OCR_REASON_SCANNED, OCR_REASON_SUSPECTED_GARBLED_TEXT,
    OCR_REASON_VECTOR_TEXT, PageOcrReasons, PdfClassification, PdfType,
};
pub use relation::{CitationSchema, RELATION_TYPES, Relation, RelationType, bibliographic};
pub use scheduler::{
    PendingTask, RetryPolicy, SchedulerTaskSchema, TaskPriority, TaskStatus, TaskType,
};
pub use schema::{
    Field, FieldType, NodeSchemaDef, SchemaError, all_node_schemas, registered_node_labels,
    registered_node_labels_or_yaml, schema_for_label,
};
pub use source::{
    SourceSchema, is_known_source_code, is_known_source_profile, is_known_source_type,
};
pub use vid::{
    Vid, entity_vid, paper_revision_vid, paper_vid, reference_vid, revision_section_vid,
    source_manifestation_vid,
};

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
/// Uses JSON-string comparison to avoid requiring PartialEq on all types.
#[cfg(test)]
pub(crate) fn roundtrip_json<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json1).expect("deserialize failed");
    let json2 = serde_json::to_string(&back).expect("re-serialize failed");
    assert_eq!(json1, json2, "roundtrip JSON mismatch");
}

/// ADR-051 §4.4 — serde roundtrip tests for all public Serialize+Deserialize types.
#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;
    use crate::evidence::SpanType;

    // ── article.rs ──

    #[test]
    fn roundtrip_section() {
        roundtrip_json(&Section {
            vid: "vid:section:abc".into(),
            title: "Introduction".into(),
            level: 1,
            order: 0,
            text: "body".into(),
            work_vid: "vid:paper:xyz".into(),
        });
    }

    #[test]
    fn roundtrip_concept() {
        roundtrip_json(&Concept {
            vid: "vid:concept:c1".into(),
            label: "GRPO".into(),
            level: 2,
            wikidata: Some("Q123".into()),
            openalex_id: None,
            works_count: 42,
            retrieval_eligible: false,
        });
    }

    #[test]
    fn roundtrip_topic() {
        roundtrip_json(&Topic {
            vid: "vid:topic:t1".into(),
            label: "NLP".into(),
            domain: Some("CS".into()),
            field: None,
            subfield: None,
            openalex_id: Some("T101".into()),
        });
    }

    #[test]
    fn roundtrip_category() {
        roundtrip_json(&Category {
            vid: "vid:cat:cs-cl".into(),
            code: "cs.CL".into(),
            name: "Computation and Language".into(),
            is_primary: true,
        });
    }

    #[test]
    fn roundtrip_author() {
        roundtrip_json(&Author {
            vid: "vid:author:a1".into(),
            name: "Jane Doe".into(),
            orcid: Some("0000-0001-2345-6789".into()),
            openalex_id: None,
            works_count: 7,
        });
    }

    #[test]
    fn roundtrip_institution() {
        roundtrip_json(&Institution {
            vid: "vid:inst:i1".into(),
            name: "MIT".into(),
            country: Some("USA".into()),
            ror: None,
            openalex_id: Some("I123".into()),
        });
    }

    #[test]
    fn roundtrip_reference() {
        roundtrip_json(&Reference {
            vid: "vid:ref:r1".into(),
            raw_text: "Smith et al. 2024".into(),
            arxiv_id: Some("2401.12345".into()),
            doi: None,
            title: Some("Title".into()),
            resolved_work_vid: None,
        });
    }

    // ── entity.rs ──

    #[test]
    fn roundtrip_entity_type_all_variants() {
        let variants = [
            EntityType::Task,
            EntityType::Method,
            EntityType::Dataset,
            EntityType::Model,
            EntityType::Metric,
            EntityType::Figure,
            EntityType::Table,
            EntityType::Equation,
            EntityType::Concept,
            EntityType::Implementation,
            EntityType::Theorem,
            EntityType::Definition,
            EntityType::Problem,
            EntityType::Motivation,
            EntityType::Gap,
            EntityType::Contribution,
            EntityType::Hypothesis,
            EntityType::Finding,
            EntityType::Mechanism,
            EntityType::Limitation,
            EntityType::FutureWork,
        ];
        for v in &variants {
            roundtrip_json(v);
        }
    }

    #[test]
    fn roundtrip_entity() {
        use crate::evidence::SourceSpan;
        roundtrip_json(&Entity {
            vid: "vid:entity:e1".into(),
            label: "GRPO".into(),
            entity_type: EntityType::Method,
            description: Some("Group Relative Policy Optimization".into()),
            source_spans: vec![SourceSpan {
                span_type: SpanType::CharOnly,
                page: None,
                bbox: None,
                char_start: Some(100),
                char_end: Some(200),
                surface: Some("GRPO".into()),
                artifact_role: "method".into(),
                artifact_hash: "abc123".into(),
                justified_char_only: true,
            }],
            confidence: 0.95,
            schema_version: 1,
        });
    }

    // ── paper.rs ──

    #[test]
    fn roundtrip_paper() {
        roundtrip_json(&Paper::new("2507.19457", "GEPA"));
    }

    #[test]
    fn roundtrip_paper_status() {
        roundtrip_json(&PaperStatus::Preprint);
        roundtrip_json(&PaperStatus::Published);
    }

    // ── eval.rs ──

    #[test]
    fn roundtrip_gold_entity() {
        roundtrip_json(&GoldEntity {
            label: "GRPO".into(),
            entity_type: "Method".into(),
            section: Some("method".into()),
        });
    }

    #[test]
    fn roundtrip_predicted_entity() {
        roundtrip_json(&PredictedEntity {
            label: "GPT-4".into(),
            entity_type: "Model".into(),
        });
    }

    #[test]
    fn roundtrip_extraction_metrics() {
        roundtrip_json(&ExtractionMetrics {
            gold_count: 10,
            predicted_count: 8,
            true_positives: 7,
            false_positives: 1,
            false_negatives: 3,
            precision: 0.875,
            recall: 0.7,
            f1: 0.777,
        });
    }

    // ── healing.rs ──

    #[test]
    fn roundtrip_healing_correct_result() {
        use kg_ontology::healing::{HealingActor, HealingOperation, ProvenanceEvent};
        roundtrip_json(&CorrectResult {
            vid: "vid:e1".into(),
            key: "label".into(),
            old_value: "old".into(),
            new_value: "new".into(),
            provenance: ProvenanceEvent::new(
                HealingOperation::Correct,
                HealingActor::Human("alice".into()),
                vec!["vid:e1".into()],
                "fix typo".into(),
            ),
        });
    }

    #[test]
    fn roundtrip_healing_merge_result() {
        use kg_ontology::healing::{HealingActor, HealingOperation, ProvenanceEvent};
        roundtrip_json(&MergeResult {
            kept_vid: "vid:e1".into(),
            merged_vid: "vid:e2".into(),
            edges_redirected: 3,
            provenance: ProvenanceEvent::new(
                HealingOperation::Merge,
                HealingActor::System("auto".into()),
                vec!["vid:e1".into(), "vid:e2".into()],
                "dedup".into(),
            ),
        });
    }

    #[test]
    fn roundtrip_healing_silence_result() {
        use kg_ontology::healing::{HealingActor, HealingOperation, ProvenanceEvent};
        roundtrip_json(&SilenceResult {
            vid: "vid:e1".into(),
            previous_eligible: true,
            provenance: ProvenanceEvent::new(
                HealingOperation::Silence,
                HealingActor::Agent("bot".into()),
                vec!["vid:e1".into()],
                "duplicate".into(),
            ),
        });
    }

    // ── evidence.rs ──

    #[test]
    fn roundtrip_source_span() {
        use crate::evidence::SourceSpan;
        roundtrip_json(&SourceSpan {
            span_type: SpanType::PageBbox,
            page: Some(1),
            bbox: Some([0.0, 0.0, 100.0, 50.0]),
            char_start: None,
            char_end: None,
            surface: Some("text".into()),
            artifact_role: "body".into(),
            artifact_hash: "hash123".into(),
            justified_char_only: false,
        });
    }

    #[test]
    fn roundtrip_evidence_assertion() {
        roundtrip_json(&EvidenceAssertion {
            claim: "GRPO improves sample efficiency".into(),
            span_type: SpanType::CharOnly,
            page: None,
            bbox: None,
            char_start: Some(10),
            char_end: Some(50),
            artifact_hash: "hash123".into(),
            artifact_path: "/data/papers/2507.19457/body.md".into(),
            epistemic_status: EpistemicStatus::Staged,
            created_at: 1700000000,
        });
    }

    #[test]
    fn roundtrip_evidence_verification() {
        roundtrip_json(&EvidenceVerification {
            node_vid: "vid:e1".into(),
            artifact_exists: true,
            hash_matches: true,
            span_in_bounds: true,
            verdict: EvidenceVerdict::Valid,
        });
    }

    // ── scheduler.rs ──

    #[test]
    fn roundtrip_pending_task() {
        roundtrip_json(&PendingTask {
            arxiv_id: "2507.19457".into(),
            task_type: crate::scheduler::TaskType::Ingest,
            added_at: 1700000000,
            retry_count: 0,
            last_retry: None,
            next_retry: 1700000060,
            status: crate::scheduler::TaskStatus::Pending,
        });
    }

    // ── relation.rs ──

    #[test]
    fn roundtrip_relation() {
        roundtrip_json(&Relation {
            source_vid: "vid:paper:p1".into(),
            target_vid: "vid:paper:p2".into(),
            relation_type: RelationType::BuildsOn,
            source_spans: vec![],
            confidence: 0.9,
            schema_version: 1,
        });
    }
}
