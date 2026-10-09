//! da-domain — pure domain types for daily-archive v2.
//!
//! Zero infrastructure dependencies. Only serde + chrono + sha2.
//! This is the single source of truth for all data shapes.
//!
//! ADR-037 §2: Domain layer depends on nothing (only std + serde).
//! ADR-038 §2: 5-module schema (A-E), 18 relation types.
//! ADR-040 §11: Schema enforcement lives here, not in DDL.

pub mod acap_policy;
pub mod acquisition_batch;
pub mod agent;
pub mod agent_manifest;
pub mod article;
pub mod article_manifest;
pub mod assurance;
pub mod boundary_agent;
pub mod catalog;
pub mod citation_identity;
pub mod cluster;
pub mod context_conservation;
pub mod corpus_checkpoint;
pub mod critic_repair;
pub mod cv_structure;
pub mod domain;
pub mod edge_contract;
pub mod enriched_authors;
pub mod entity;
pub mod entity_resolution;
pub mod equation_candidate;
pub mod eval;
pub mod evidence;
pub mod evidence_bundle;
pub mod extractive_summary;
pub mod feature_abi;
pub mod figure;
pub mod figure_analysis;
pub mod figure_candidate;
pub mod figure_context;
pub mod figure_geometry;
pub mod finalize;
pub mod fsm_policy;
pub mod graph_tasks;
pub mod healing;
pub mod html_headings;
pub mod hypergraph;
pub mod influence_flower;
pub mod lexicon;
pub mod load;
pub mod materialization_profile;
pub mod navigation;
pub mod negative_memory;
pub mod ordered_mutation;
pub mod paper;
pub mod parse_job;
pub mod pdf_toc;
pub mod pipeline_cache_lineage;
pub mod pipeline_ledger;
pub mod pipeline_metrics;
pub mod process;
pub mod promotion;
pub mod prompt_optimization;
pub mod relation;
pub mod revision_classification;
pub mod revision_readiness;
pub mod revision_schema;
pub mod scalar_crf;
pub mod scheduler;
pub mod schema;
pub mod schema_invariants;
pub mod scholarly_evidence;
pub mod section_summary_policy;
pub mod source;
pub mod source_import_fsm;
pub mod stage_invariants;
pub mod structural_elements;
pub mod summaries_llm;
pub mod table_fidelity;
pub mod tei_figures;
pub mod temporal;
pub mod tex_source;
pub mod validator;
pub mod vid;
pub mod vocabulary;
pub mod xy_cut;
pub mod yaml_schema_adapter;

pub use acap_policy::{ACAP_FSM_ORDER, AcapAdmissionPolicy, AcapForwardRule};
pub use agent::{
    AgentAdmissionDecision, AgentAdmissionError, AgentBudget, AgentBudgetError, AgentClaim,
    AgentFailureCode, AgentGroundingError, AgentQueryEvidence, AgentState, ContextBundle,
    ContextFreeze, GraphIdentity, GraphIdentityError, admit_transition as agent_admit_transition,
    verify_identity as agent_verify_identity,
};
pub use article::{
    Author, AuthorSchema, Category, CategorySchema, Concept, ConceptSchema, Institution,
    InstitutionSchema, Reference, ReferenceSchema, Section, SectionSchema, Topic, TopicSchema,
};
pub use assurance::{
    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ACAP_POLICY_ID, ACAP_REPORT_SCHEMA_VERSION,
    AdmissionDecision, ArticleAssuranceReport, ArticleContextState, AssuranceError,
    AssuranceVerdict, BoundaryMode, ConservationObservation, ContentDigest, CorpusAggregateMetrics,
    CorpusAssuranceReport, CorpusLayerResult, CorpusLayerStatus, CountObservation,
    LayerAssuranceResult, MissingnessReason, MutationFamilyObservation, ReplayObservation,
    TransitionAdmissionCode, TransitionAdmissionError, WriteDelta, acap_skip_reason_for,
    admit_stage_transition, admit_stage_transition_with_policy, admit_transition,
    corpus_report_digest,
};
pub use citation_identity::{
    AmbiguousCitationReason, CitationEntry, CitationOccurrence, CitationTarget, CitationWorkLink,
    ReadingOrder, ReadingOrderConstraint, UnresolvedCitationReason, UnresolvedOrderReason,
    citation_entry_vid, citation_occurrence_vid,
};
pub use edge_contract::{
    EdgeContract, contract_edge_types, contract_edge_types_or_yaml, edge_contracts,
    edge_contracts_or_yaml, render_edge_contracts_yaml, render_markdown_table,
    validate_compiled_edge_endpoint,
};
pub use entity::{ENTITY_TYPES, Entity, EntitySchema, EntityType};
pub use equation_candidate::{
    EQUATION_CANDIDATE_SCHEMA_VERSION, EquationCandidate, EquationCandidateError,
    EquationCandidateInput, EquationRepresentationKind, equation_candidate_vid,
};
pub use eval::{
    ExtractionMetrics, GoldEntity, MultiPaperGoldIndex, PredictedEntity, list_false_positives,
    promote_candidates,
};
pub use evidence::{
    EpistemicStatus, EvidenceAssertion, EvidenceId, EvidenceVerdict, EvidenceVerification,
    SourceSpan, SpanType,
};
pub use feature_abi::{FeatureAbi, FeatureAbiError, FeatureChannel, FeatureToken, FeatureVector};
pub use figure::{
    BoundingBox72Dpi, DetectedVisualArtifact, VisualArtifactError, VisualArtifactKind,
};
pub use figure_analysis::{
    FigureAnalysis, FigureAnalysisCitation, FigureAnalysisComponent, FigureAnalysisError,
    FigureAnalysisProvenance, FigureAnalysisRelationship, FigureChartAxis, FigureChartDataPoint,
    FigureChartPayload, FigureClassifyMethod, FigurePanelMarkerTable, FigurePhotoPayload,
    FigureScaleBar, FigureSchemaRegistry, FigureSchemaSelection, FigureTablePayload,
    FigureTypeClassification, FigureTypeLabelTable, FigureTypedPayload, VisualAnalysisType,
};
pub use figure_candidate::{FigureCandidate, FigureCandidateError, figure_candidate_vid};
pub use figure_context::{
    ArticleSummaryContext, FigureContextError, FigureReferenceExcerpt, FigureSectionContext,
    FigureSectionRelation, FigureSemanticContext,
};
pub use healing::{
    CorrectResult, HealingActor, HealingOperation, MergeResult, ProvenanceEvent, SilenceResult,
};
pub use hypergraph::ConceptClusterSchema;
pub use paper::{Paper, PaperSchema, PaperStatus};
pub use pipeline_ledger::{
    PipelineStageContract, PipelineStageLedger, StageCacheMode, StageId, StageIdRegistry,
    StageOwner,
};
pub use promotion::{
    ActorId, EvidenceRef, PromotionCandidate, PromotionError, PromotionKind, PromotionState,
};
pub use relation::hypergraph::{
    CONTRADICTS, MEMBER_OF_CLUSTER, PARTICIPATES_IN, QUALIFIES, SUPPORTS,
};
pub use stage_invariants::{
    AUTHORITATIVE_FALSE, CANDIDATE_NOT_AUTHORITATIVE, CANDIDATE_NOT_GRAPH_TRUTH, D127_FALSE,
    DATA_POLICY_BOUND, DERIVED_NOT_AUTHORITATIVE, DispositionKind, EVIDENCE_REQUIRED,
    GRAPH_MATERIALIZATION_NEVER_SKIPPED_BY_COMPUTE_CACHE, IMPLEMENTATION_AND_CONFIG_IN_CACHE_KEY,
    IMPORT_ELIGIBLE_FALSE, MODEL_AND_DIMENSIONS_BOUND, NAMED_STAGE_INVARIANTS, NO_NAN,
    OUTPUT_SCHEMA_BOUND, PACKAGE_DIGEST_BOUND, PARSER_PROFILE_IN_CONFIG_FINGERPRINT,
    POLICY_DIGEST_BOUND, REPLAY_IDEMPOTENT, REPORT_LINEAGE_BOUND, SOURCE_CHECKSUM_IN_CACHE_KEY,
    STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION, StageInvariantDisposition,
    StageInvariantDispositions, StageInvariantFacts, StageInvariantObservation,
    StageInvariantViolation, TYPED_MISSINGNESS, UNKNOWN_INVARIANT_REASON, evaluate_stage_invariant,
    stage_invariant_is_named, validate_stage_invariant_dispositions,
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
pub use influence_flower::{
    CitationInfluenceEvent, InfluenceFlowerPolicy, accumulate_author_influence, influence_weight,
};
pub use lexicon::{
    FeatureTemplate, GROBID_LEXICON_SCHEMA_VERSION, GROBID_RESOURCE_VERSION, LexiconBundle,
    LexiconChannel, LexiconError, MAX_FEATURE_TEMPLATES, MAX_LEXICON_CHANNELS,
    MAX_LEXICON_ENTRIES_PER_CHANNEL, MAX_TEMPLATE_INPUTS,
};
pub use load::{
    AcquisitionPolicy, ArticleLocator, AssetDisposition, LOAD_REPORT_SCHEMA_VERSION,
    LoadAssetRecord, LoadCacheRecord, LoadDomainError, LoadFinding, LoadQueueRecord,
    LoadQueueTaskRef, LoadReport, LoadRunMode, LoadStageName, LoadStageSet, PrimaryContentKind,
    StageStatus, locator_implies_network, try_normalize_arxiv_id, try_parse_arxiv_locator,
};
pub use pdf_classification::{
    OCR_REASON_NO_TEXT, OCR_REASON_SCANNED, OCR_REASON_SUSPECTED_GARBLED_TEXT,
    OCR_REASON_VECTOR_TEXT, PageOcrReasons, PdfClassification, PdfTriageDecision, PdfTriagePolicy,
    PdfTriageStatus, PdfType, TRIAGE_CONFIDENCE_EPSILON, TriageModeDiff,
};
pub use relation::{CitationSchema, RELATION_TYPES, Relation, RelationType, bibliographic};
pub use revision_classification::{
    RevisionClass, RevisionClassification, RevisionClassificationPolicy, classify_revision,
};
pub use scheduler::{
    AttemptError, AttemptOutcome, DEFAULT_LEASE_TERM_SECS, DependencyNode, HEARTBEAT_INTERVAL_SECS,
    LeaseError, LogicalTaskResult, MAX_CLOCK_DRIFT_SECS, MAX_LEASE_TERM_SECS, PendingTask,
    PublishError, RetryPolicy, SchedulerTaskSchema, TaskAttempt, TaskDependency, TaskLease,
    TaskPriority, TaskRecordError, TaskResourceClass, TaskStatus, TaskType,
    all_dependencies_satisfied, find_dependency_cycle, logical_result_vid,
};
pub use schema::{
    Field, FieldType, NodeSchemaDef, SchemaError, all_node_schemas, registered_node_labels,
    registered_node_labels_or_yaml, schema_for_label,
};
pub use source::{
    SourceSchema, is_known_source_code, is_known_source_profile, is_known_source_type,
};
pub use table_fidelity::{
    StructuralTable, StructuralTableCell, TableCellCoverage, TableFidelityError,
    TableFidelityFinding, TableFidelityMissingness, TableFidelityReport, compare_table_fidelity,
};
pub mod table_candidate;
pub use table_candidate::{
    TABLE_CANDIDATE_SCHEMA_VERSION, TableCandidate, TableCandidateError, TableCandidateLineage,
    TableCellCandidate, TableConflictEvidence, fidelity_kind, table_candidate_vid, table_cell_vid,
    table_conflict_vid,
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
            schema_version: 2,
            priority: crate::scheduler::TaskPriority::High,
            lease: None,
            dependencies: vec![],
            resource_class: crate::scheduler::TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
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

    // ─── ADR-051 §4.4: serde roundtrip for M386/M387 types ───

    #[test]
    fn roundtrip_prompt_candidate() {
        use crate::prompt_optimization::{PromptCandidate, PromptCandidateState};
        roundtrip_json(&PromptCandidate {
            candidate_id: "c1".into(),
            task_id: "summary".into(),
            state: PromptCandidateState::Evaluated,
            baseline_spec_sha256: ContentDigest::of_text("base"),
            candidate_spec_sha256: Some(ContentDigest::of_text("candidate")),
            evaluation: None,
            human_decision_ref: None,
            deployed: false,
            import_eligible: false,
        });
    }

    #[test]
    fn roundtrip_prompt_candidate_state_all_variants() {
        use crate::prompt_optimization::PromptCandidateState;
        let states = [
            PromptCandidateState::BaselineRegistered,
            PromptCandidateState::CandidateProposed,
            PromptCandidateState::Evaluated,
            PromptCandidateState::Eligible,
            PromptCandidateState::Rejected,
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let back: PromptCandidateState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, back);
        }
    }

    // ── promotion.rs (PIR-015) ──

    #[test]
    fn roundtrip_promotion_candidate() {
        use crate::promotion::EvidenceRef;
        roundtrip_json(&PromotionCandidate {
            candidate_id: "cand-1".into(),
            kind: crate::promotion::PromotionKind::EntityResolution,
            state: crate::promotion::PromotionState::Reviewed,
            proposer: "alice".into(),
            verifier: Some("bob".into()),
            reviewer: Some("carol".into()),
            promoter: None,
            evidence_refs: vec![EvidenceRef::new("ev:review:run-43").with_kind("gsd_uat_exec")],
            rejection_reason: None,
            import_eligible: false,
        });
    }

    #[test]
    fn roundtrip_promotion_kind_all_variants() {
        use crate::promotion::PromotionKind;
        let kinds = [
            PromotionKind::Prompt,
            PromotionKind::Gold,
            PromotionKind::EntityResolution,
        ];
        for kind in &kinds {
            let json = serde_json::to_string(kind).unwrap();
            let back: PromotionKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*kind, back);
        }
    }

    #[test]
    fn roundtrip_promotion_state_all_variants() {
        use crate::promotion::PromotionState;
        let states = [
            PromotionState::Proposed,
            PromotionState::Verified,
            PromotionState::Reviewed,
            PromotionState::Promoted,
            PromotionState::Rejected,
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let back: PromotionState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, back);
        }
    }

    // ── stage_invariants.rs (S08/T01) ──

    #[test]
    fn roundtrip_stage_invariant_facts() {
        roundtrip_json(&StageInvariantFacts {
            import_eligible: Some(false),
            d127_unlock_count: Some(0),
            replay_net_writes: Some(0),
            authoritative: Some(false),
            graph_truth_boundary: Some(false),
            vectors_all_finite: Some(true),
            missingness_typed: Some(true),
            model_id: Some("bge-m3".into()),
            dimensions: Some(1024),
            output_schema_fingerprint: Some("schema:v1".into()),
            configuration_fingerprint: Some("config:v1".into()),
            cache_key_fields: vec!["source_sha256".into()],
            evidence_keys: ["policy_digest".to_string()].into_iter().collect(),
        });
    }

    #[test]
    fn roundtrip_stage_invariant_facts_default() {
        // The all-absent default is the fail-closed shape and must roundtrip.
        roundtrip_json(&StageInvariantFacts::default());
    }

    #[test]
    fn roundtrip_stage_invariant_observation() {
        roundtrip_json(&StageInvariantObservation::new(
            "extraction_materialization",
            StageCacheMode::GraphMaterialize,
            true,
        ));
    }

    #[test]
    fn roundtrip_stage_invariant_violation() {
        roundtrip_json(&StageInvariantViolation {
            invariant: "import_eligible_false".into(),
            stage_id: "parse".into(),
            reason: "import_eligible_not_false".into(),
        });
    }

    #[test]
    fn roundtrip_disposition_kind_all_variants() {
        roundtrip_json(&DispositionKind::Checked);
        roundtrip_json(&DispositionKind::Downgraded);
    }

    #[test]
    fn roundtrip_stage_invariant_dispositions() {
        use std::collections::BTreeMap;
        let mut records = BTreeMap::new();
        records.insert(
            "evidence_required".to_string(),
            StageInvariantDisposition {
                disposition: DispositionKind::Checked,
                check_id: Some("evidence_required".into()),
                owner: None,
                reason: None,
                decision_ref: None,
                rationale: None,
            },
        );
        records.insert(
            "readonly_policy".to_string(),
            StageInvariantDisposition {
                disposition: DispositionKind::Downgraded,
                check_id: None,
                owner: Some("M516-th0oby/S08".into()),
                reason: Some("review_only_commitment".into()),
                decision_ref: Some("D556".into()),
                rationale: Some("no runtime predicate exists today".into()),
            },
        );
        let mut reason_vocabulary = BTreeMap::new();
        reason_vocabulary.insert(
            "review_only_commitment".to_string(),
            "review adjudicates it".to_string(),
        );
        roundtrip_json(&StageInvariantDispositions {
            schema_version: STAGE_INVARIANT_DISPOSITIONS_SCHEMA_VERSION.into(),
            reason_vocabulary,
            records,
        });
    }
}
