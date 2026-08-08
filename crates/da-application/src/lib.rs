//! da-application — use cases and orchestrators.
//!
//! ADR-037 §2: Application layer depends on Domain + Ports.
//! ADR-037 §4: Data flows (ingest, extraction, agent, ETL scheduler).
//! ADR-040 §11.3: Migration framework.

pub mod artifact_stage;
pub mod ingest;
pub mod pipeline;

pub use artifact_stage::ArtifactPayloadStage;
pub use ingest::IngestUseCase;
pub use pipeline::{
    PipelineOrchestrator, PipelineOrchestratorError, PipelineRunReport, StageDisposition,
    StageRunRecord,
};
pub mod batch_ingest;
pub use batch_ingest::{BatchIngestResult, BatchPdfTriageSummary, batch_ingest_pdfs};
pub mod entity_resolution;
pub mod extraction;
pub use entity_resolution::{
    EntityResolutionError, EntityResolutionUseCase, ReviewedMergeRequest, ReviewedMergeResult,
};
pub use extraction::{ExtractionResult, ExtractionUseCase};
pub mod figure_analysis;
pub use figure_analysis::{
    FIGURE_ANALYSIS_PROMPT_VERSION, FIGURE_ANALYSIS_SCHEMA_VERSION, FigureAnalysisUseCase,
    FigureAnalysisUseCaseError,
};
pub mod figure_analysis_stage;
pub use figure_analysis_stage::{FigureAnalysisStage, FigureContextArtifactStage};
pub mod figure_context;
pub use figure_context::{FigureContextAssembler, FigureContextBudget, FigureSectionLocator};
pub mod figure_context_stage;
pub use figure_context_stage::FigureContextStage;
pub mod healing;
pub use healing::GraphHealingUseCase;
pub mod enrich;
pub use enrich::{EnrichResult, EnrichUseCase};
pub mod scheduler;
pub mod scheduler_runner;
pub use scheduler::GraphScheduler;
pub use scheduler_runner::{
    SchedulerRunReport, SchedulerRunner, SchedulerTaskDisposition, SchedulerTaskOutcome,
};
pub mod cluster;
pub mod cluster_stage;
pub mod document_context_stage;
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
pub use schema_init::{CURRENT_SCHEMA_VERSION, SchemaInitializer};
pub mod decision_recorder;
pub use decision_recorder::{
    DecisionRecord, DecisionRecordError, DecisionRecorder, DecisionRequest, DecisionRequestBuilder,
};
pub mod graph_inventory;
pub use graph_inventory::{GraphInventory, collect_graph_inventory, render_inventory_markdown};
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
            citation_count: 42,
            references_written: 42,
            cites_resolved: 35,
            vector_dimensions: 1024,
            graph_node_id: Some(123),
            import_eligible: false,
            pdf_classification: None,
        });
        // M350: with classification attached
        roundtrip_json(&IngestResult {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            body_chars: 50000,
            section_count: 8,
            citation_count: 42,
            references_written: 42,
            cites_resolved: 35,
            vector_dimensions: 1024,
            graph_node_id: Some(123),
            import_eligible: false,
            pdf_classification: Some(da_domain::PdfClassification {
                pdf_type: da_domain::PdfType::Text,
                page_count: 96,
                pages_needing_ocr: vec![69, 84],
                ocr_reasons_by_page: vec![],
                confidence: 1.0,
                has_encoding_issues: true,
            }),
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
