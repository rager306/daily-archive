//! da-application — use cases and orchestrators.
//!
//! ADR-037 §2: Application layer depends on Domain + Ports.
//! ADR-037 §4: Data flows (ingest, extraction, agent, ETL scheduler).
//! ADR-040 §11.3: Migration framework.

pub mod agent_run;
pub mod article_load;
pub mod artifact_stage;
pub mod assurance;
pub mod candidate_materialization_stage;
pub mod catalog_reconcile;
pub mod embedding_write;
pub mod equation_extraction_stage;
pub use embedding_write::{NodeEmbedWrite, write_node_embedding};
pub mod extraction_materialization_stage;
pub mod ingest;
pub mod pipeline;
pub mod revision_classification;
pub mod revision_materialization;
pub mod revision_materialization_stage;
pub mod revision_query;
pub mod revision_readiness;
pub mod table_extraction_stage;
pub mod tex_inspect;

pub use agent_run::{
    AgentPlanQuery, AgentRunReport, AgentRunRequest, AgentRunUseCase, DeterministicSynthesizer,
    claims_digest, evidence_digest, plan_digest, verification_digest, verify_run_evidence_chain,
};
pub use article_load::{
    ArticleLoadError, ArticleLoadUseCase, LoadContentOutcome, LoadContentPipeline,
    LoadContentRequest, LoadExtractOutcome, LoadIngestOutcome,
};
pub use artifact_stage::ArtifactPayloadStage;
pub use assurance::{
    MaterializationAssuranceUseCase, MaterializationReadback, MaterializationReplayObservation,
    MaterializedSectionObservation,
};
pub use catalog_reconcile::CatalogReconcileUseCase;
pub use extraction_materialization_stage::{
    ExtractionMaterializationReport, ExtractionMaterializationStage,
};
pub use ingest::{IngestUseCase, embedding_fingerprint};
pub use pipeline::{
    ArticleJourneyComposition, MultimodalComposition, MultimodalFingerprintSummary,
    PipelineOrchestrator, PipelineOrchestratorError, PipelineRunReport, StageDisposition,
    StageRunRecord,
};
pub use revision_classification::{RevisionClassifyError, RevisionClassifyUseCase};
pub use revision_materialization::{
    RevisionContentContext, RevisionManifestationInput, RevisionMaterializationError,
    RevisionMaterializationRequest, RevisionMaterializationResult, RevisionMaterializationUseCase,
};
pub use revision_materialization_stage::{
    RevisionMaterializationReport, RevisionMaterializationStage,
};
pub use revision_query::{
    ArticleRevisionView, RevisionManifestationView, RevisionQueryError, RevisionQueryResponse,
    RevisionQueryUseCase, RevisionSelector,
};
pub use tex_inspect::TexInspectUseCase;
pub mod batch_ingest;
pub use batch_ingest::{
    BatchIngestResult, BatchPdfTriageSummary, batch_ingest_pdfs, batch_ingest_pdfs_with_context,
};
pub mod critic_repair_stage;
pub mod cross_layer_signals;
pub mod enriched_layer_stage;
pub mod entity_resolution;
pub mod extraction;
pub mod extractive_summary;
pub mod figures_stage;
pub mod fsm_policy_consult;
pub mod fsm_policy_fallback;
pub mod html_parsed_stage;
pub mod navigation_stage;
pub mod parse_grammar;
pub mod parse_learning_evaluation;
pub mod parse_policy_training;
pub mod pdf_toc_stage;
pub mod sona_bridge;
pub use da_ports::CandidateManifest;
pub use parse_learning_evaluation::{
    ABLATION_SCHEMA_VERSION, AblationReport, CRITIC_SCHEMA_VERSION, CriticObservation,
    CriticStatus, EvaluationError, FrozenEvalCase, aggregate_same_n_ablation,
};
pub use parse_policy_training::{
    PersistedCandidate, TrainingConfig, TrainingError, TrainingOutcome, train_microlora_candidate,
};
pub mod structural_elements_stage;
pub mod summaries_agent_stage;
pub mod tei_figures_stage;
pub mod validate_schema_objects;
pub use entity_resolution::{
    EntityResolutionError, EntityResolutionUseCase, ReviewedMergeRequest, ReviewedMergeResult,
    SystemExactAliasApplied, SystemExactAliasBatchResult, SystemExactAliasFailure,
    SystemExactAliasSkip,
};
pub mod promotion;
pub use extraction::{ExtractionResult, ExtractionUseCase};
pub use extractive_summary::{SummaryAsOf, get_summary_as_of, list_summary_versions};
pub use promotion::{
    PromotionAction, PromotionService, PromotionServiceError, PromotionTransitionRequest,
};
pub mod figure_analysis;
pub mod figure_candidate_assembly;
pub use figure_analysis::{
    FIGURE_ANALYSIS_PROMPT_VERSION, FIGURE_ANALYSIS_SCHEMA_VERSION, FigureAnalysisUseCase,
    FigureAnalysisUseCaseError,
};
pub mod figure_classify;
pub use figure_classify::{FigureClassifyError, FigureClassifyUseCase};
pub mod figure_schema;
pub use figure_schema::{FigureSchemaError, FigureSchemaService};
pub mod figure_analysis_stage;
pub use figure_analysis_stage::{FigureAnalysisStage, FigureContextArtifactStage};
pub mod figure_context;
pub use figure_context::{
    FigureContextAssembler, FigureContextBudget, FigureRevisionIdentity, FigureSectionLocator,
};
pub mod figure_context_stage;
pub use figure_context_stage::FigureContextStage;
pub mod figure_materialize;
pub use figure_materialize::{
    DIAGNOSTIC_APPLIED, DIAGNOSTIC_UNCHANGED_REPLAY, FigureCandidateMaterializer,
    FigureMaterializationError, FigureMaterializationResult,
};
pub mod table_materialize;
pub use table_materialize::{
    TableCandidateMaterializer, TableMaterializationError, TableMaterializationResult,
    structural_table_from_extracted,
};
pub mod equation_materialize;
pub use equation_materialize::{
    EQUATION_CANDIDATE_LABEL, EquationCandidateMaterializer, EquationMaterializationError,
    EquationMaterializationResult, equation_candidate_input_from_extracted,
};
pub mod healing;
pub use healing::GraphHealingUseCase;
pub mod catalog_materialization;
pub mod enrich;
pub mod materialization_indexes;
pub use enrich::{EnrichResult, EnrichUseCase};
pub mod influence_flower;
pub use influence_flower::{
    InfluenceFlower, InfluenceFlowerError, InfluencePetal, build_author_flower,
};
pub mod catalog_pdf_locator;
pub use catalog_pdf_locator::FilesystemCatalogPdfLocator;
pub mod scheduler;
pub mod scheduler_executors;
pub mod scheduler_runner;
pub use scheduler::GraphScheduler;
pub use scheduler_executors::{
    EmbedStaleSchedulerExecutor, ExtractSchedulerExecutor, IngestSchedulerExecutor,
    ReparseSchedulerExecutor,
};
pub use scheduler_runner::{
    SchedulerExecuteOutcome, SchedulerRunReport, SchedulerRunner, SchedulerTaskDisposition,
    SchedulerTaskExecutor, SchedulerTaskOutcome,
};
pub mod cluster;
pub mod cluster_stage;
pub mod derived_evidence_maturity;
pub mod derived_family_adapters;
pub mod derived_prompt_contract;
pub mod derived_transition_evaluator;
pub mod document_context_stage;
pub mod native_family_decision_packages;
pub mod native_family_promotion_gate;
pub mod native_family_same_n_adapter;
pub mod scholarly_evidence;
pub mod summary;
pub use cluster::{ClusterResult, ClusterUseCase};
pub use cluster_stage::{ClusterDetectionStage, EntityPapersArtifact, EntityPapersEntry};
pub use document_context_stage::{
    DocumentContextProjection, DocumentContextSection, DocumentContextStage, SectionSummaryInput,
};
pub use summary::{SummaryResult, SummaryUseCase};
pub mod queries;
pub use queries::{
    EntityQueries, PaperLookupError, PaperQueries, RelationQueries, find_paper_node_id,
};
pub mod schema_init;
pub use schema_init::CURRENT_SCHEMA_VERSION;
pub mod section_navigation;
pub use section_navigation::{
    NavDiagCode, NavDiagnostic, NavigationReport, render_navigation_report,
    validate_section_navigation,
};
pub mod decision_recorder;
pub use decision_recorder::{
    DecisionRecord, DecisionRecordError, DecisionRecorder, DecisionRequest, DecisionRequestBuilder,
};
pub mod acap_contract_audit;
pub mod acquisition_batch;
pub mod context_conservation;
pub mod corpus_assurance;
pub mod corpus_checkpoint;
pub mod graph_finalize;
pub mod graph_inventory;
pub mod identity_compatibility;
pub mod ontology_contract_audit;
pub mod ontology_endpoint_evidence;
pub mod ontology_v2_convergence;
pub mod ordered_mutation;
pub mod pipeline_cache_audit;
pub mod pipeline_ledger_audit;
pub mod pipeline_metrics;
pub mod pipeline_stage_assurance;
pub mod prompt_evaluation;
pub use graph_finalize::GraphFinalizeUseCase;
pub use graph_inventory::{
    GraphInventory, collect_graph_inventory, collect_graph_inventory_for_registry,
    render_inventory_markdown,
};
pub mod temporal_stamp;
pub use temporal_stamp::{
    create_edge_with_temporal, edge_type_is_temporal_fact, open_fact_edge_now,
    stamp_edge_temporal_fields,
};

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
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

#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn roundtrip_ingest_result() {
        use crate::ingest::IngestResult;
        roundtrip_json(&IngestResult {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            body_chars: 50000,
            section_count: 8,
            sections_written: 8,
            citation_count: 42,
            references_written: 42,
            cites_resolved: 35,
            vector_dimensions: 1024,
            graph_node_id: Some(123),
            revision_vid: None,
            import_eligible: false,
            pdf_classification: None,
            pdf_triage: da_domain::PdfTriageStatus::NotConfigured,
            page_text_missingness: None,
        });
        // M350: with classification attached
        roundtrip_json(&IngestResult {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            body_chars: 50000,
            section_count: 8,
            sections_written: 8,
            citation_count: 42,
            references_written: 42,
            cites_resolved: 35,
            vector_dimensions: 1024,
            graph_node_id: Some(123),
            revision_vid: None,
            import_eligible: false,
            pdf_classification: Some(da_domain::PdfClassification {
                pdf_type: da_domain::PdfType::Text,
                page_count: 96,
                pages_needing_ocr: vec![69, 84],
                ocr_reasons_by_page: vec![],
                confidence: 1.0,
                has_encoding_issues: true,
            }),
            pdf_triage: da_domain::PdfTriageStatus::Classified,
            page_text_missingness: None,
        });
    }

    #[test]
    fn roundtrip_batch_ingest_result() {
        roundtrip_json(&BatchIngestResult {
            total: 24,
            ok: 24,
            fail: 0,
            total_body_chars: 1_200_000,
            total_sections: 192,
            total_citations: 1008,
            total_cites_resolved: 840,
            total_references_written: 1008,
            duration_ms: 45000,
            snapshot_path: Some("/tmp/hub.sgsnap".into()),
            errors: vec![],
            import_eligible: false,
            pdf_triage: BatchPdfTriageSummary::default(),
        });
    }

    #[test]
    fn roundtrip_batch_ingest_result_with_errors() {
        roundtrip_json(&BatchIngestResult {
            total: 3,
            ok: 2,
            fail: 1,
            total_body_chars: 100000,
            total_sections: 16,
            total_citations: 80,
            total_cites_resolved: 60,
            total_references_written: 80,
            duration_ms: 5000,
            snapshot_path: None,
            errors: vec![("2401.99999".into(), "parse failed".into())],
            import_eligible: false,
            pdf_triage: BatchPdfTriageSummary {
                classified: 2,
                with_ocr_pages: 1,
                encoding_issues: 1,
                type_text: 2,
                type_scanned: 0,
                type_image: 0,
                type_mixed: 0,
                type_unknown: 0,
                papers_with_ocr: vec!["2507.19457".into()],
            },
        });
    }

    /// M353: shape lock — BatchIngestResult JSON must expose pdf_triage key.
    #[test]
    fn batch_ingest_result_json_has_pdf_triage_key() {
        let r = BatchIngestResult {
            total: 1,
            ok: 1,
            fail: 0,
            total_body_chars: 1,
            total_sections: 0,
            total_citations: 0,
            total_cites_resolved: 0,
            total_references_written: 0,
            duration_ms: 1,
            snapshot_path: None,
            errors: vec![],
            import_eligible: false,
            pdf_triage: BatchPdfTriageSummary::default(),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert!(
            v.get("pdf_triage").is_some(),
            "BatchIngestResult must serialize pdf_triage (M352/M353 shape lock)"
        );
    }

    #[test]
    fn roundtrip_graph_inventory() {
        let mut node_counts = BTreeMap::new();
        node_counts.insert("Paper".into(), 24);
        node_counts.insert("Entity".into(), 96);
        node_counts.insert("Reference".into(), 0);
        let mut edge_hist = BTreeMap::new();
        edge_hist.insert("CITES".into(), 2741);
        edge_hist.insert("hasPart".into(), 192);
        roundtrip_json(&GraphInventory {
            node_counts,
            edge_type_histogram: edge_hist,
            total_nodes: 120,
            total_edges: 2933,
        });
    }
}
