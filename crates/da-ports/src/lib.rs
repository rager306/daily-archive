//! da-ports — trait definitions (ports in hexagonal architecture).
//!
//! ADR-037 §2: Ports layer depends on Domain only.
//! These traits define the boundaries between our application and external systems.

pub mod acquisition;
pub mod algorithms;
pub mod assurance;
pub mod cache_payload;
pub mod catalog;
pub mod clock;
pub mod embedder;
pub mod evidence_store;
pub mod extractor;
pub mod figure_detection;
pub mod graph_store;
pub mod llm_client;
pub mod multimodal_llm;
pub mod openalex;
pub mod parser;
pub mod pipeline_cache;
pub mod pipeline_stage;
pub mod promotion;
pub mod prompt_execution;
pub mod tex_source;

pub use acquisition::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, MockSourceAcquisition,
    ResolvedArticleRef, SourceAcquisitionPort,
};
pub use algorithms::GraphAlgorithms;
pub use assurance::{AssuranceReportStore, AssuranceStoreError};
pub use cache_payload::{
    CachePayloadError, EXTRACTED_ENTITY_SCHEMA_ID, decode_extracted_entities,
    extracted_entity_json_schema, extracted_entity_output_schema_fingerprint,
};
pub use catalog::{
    CatalogInventoryError, CatalogInventoryPort, CatalogInventorySnapshot, CatalogMutationPort,
    CatalogWriteDisposition,
};
pub use clock::{Clock, FixedClock, SystemClock};
pub use embedder::Embedder;
pub use evidence_store::EvidenceStore;
pub use extractor::{ExtractedEntity, Extractor};
pub use figure_detection::{
    FigureDetectionError, FigureDetectionPort, FigureDetectionPortResult, FigureDetectionRequest,
    FigureDetectionResult, validate_revision_vid,
};
pub use graph_store::{DirectGraphStore, GraphStore, MutationCounters, MutationFamilyCounters};
pub use llm_client::{ChatMessage, ChatOptions, ChatResponse, LLMClient};
pub use multimodal_llm::{
    ImageMediaType, ModelRouteDescriptor, ModelWireProtocol, MultimodalImage, MultimodalLlmError,
    MultimodalLlmPort, MultimodalLlmRequest, MultimodalLlmResponse, MultimodalLlmResult,
};
pub use openalex::{OpenAlexAuthor, OpenAlexClient, OpenAlexConcept, OpenAlexTopic, OpenAlexWork};
pub mod pdf_classification;
pub use pdf_classification::{
    ClassificationError, ClassificationResult, NullClassifier, PdfClassificationPort,
};

pub use parser::{CitationAnchor, CitationEntry, ParserPort, Section};
pub use promotion::{PromotionCommitError, PromotionCommitSink, PromotionDecision};
pub use tex_source::{
    TexAnalysisReportStore, TexAnalysisRequest, TexArchiveError, TexArchiveInventory,
    TexArchiveMember, TexArchivePort,
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
    use crate::parser::{CitationEntry, ParsedArticle, Section};

    #[test]
    fn roundtrip_parsed_article() {
        roundtrip_json(&ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "abstract".into(),
            body_text: "body".into(),
            sections: vec![Section {
                title: "Intro".into(),
                text: "text".into(),
                level: 1,
                ..Default::default()
            }],
            citations: vec![],
            layout_json: Some(serde_json::json!({"pages": 10})),
            tei_xml: None,
            pdf_hash: "abc123".into(),
        });
    }

    #[test]
    fn roundtrip_section() {
        roundtrip_json(&Section {
            title: "Methods".into(),
            text: "we propose".into(),
            level: 2,
            ..Default::default()
        });
    }

    /// ADR-053 Layer A (M362-d4qoaf/S01 T01): verify that the new navigation
    /// fields (path_id, parent_section_id, order_in_parent, section_id)
    /// survive serde roundtrip — they must be present and stable for the
    /// ingest use case (T04) to build first_child / next_sibling / parent_section
    /// edges.
    #[test]
    fn roundtrip_section_with_navigation_fields() {
        roundtrip_json(&Section {
            title: "Setup".into(),
            text: "experimental setup".into(),
            level: 3,
            path_id: Some("3.2.1".to_string()),
            parent_section_id: Some("sec-3.2".to_string()),
            order_in_parent: Some(0),
            section_id: Some("sec-3.2.1".to_string()),
        });
    }

    /// Backward compat: legacy parsers produce Section without navigation
    /// fields. Serde default must tolerate missing fields on deserialize.
    #[test]
    fn roundtrip_section_legacy_omits_navigation_fields() {
        let json = r#"{"title":"Intro","text":"body","level":1}"#;
        let section: Section = serde_json::from_str(json).expect("legacy Section deserializes");
        assert_eq!(section.title, "Intro");
        assert_eq!(section.text, "body");
        assert_eq!(section.level, 1);
        assert!(section.path_id.is_none());
        assert!(section.parent_section_id.is_none());
        assert!(section.order_in_parent.is_none());
        assert!(section.section_id.is_none());
    }

    #[test]
    fn roundtrip_citation_entry() {
        roundtrip_json(&CitationEntry {
            raw_text: "Smith et al. 2024".into(),
            doi: Some("10.1234/abc".into()),
            arxiv_id: None,
            title: Some("Title".into()),
            anchors: vec![],
        });
    }

    /// T03 (M398-cz4pzx/S03): in-text citation anchors survive serde
    /// roundtrip — ref_id, section_path_id, quote and offset must be stable
    /// on the wire so the ingest layer can persist them on revision-owned
    /// edges (D247/D249).
    #[test]
    fn roundtrip_citation_entry_with_anchors() {
        roundtrip_json(&CitationEntry {
            raw_text: "Smith et al. 2024".into(),
            doi: None,
            arxiv_id: Some("2401.00001".into()),
            title: None,
            anchors: vec![
                CitationAnchor {
                    ref_id: "b12".into(),
                    section_path_id: Some("3.2".into()),
                    quote: "[12]".into(),
                    offset: Some(41),
                },
                CitationAnchor {
                    ref_id: "b12".into(),
                    section_path_id: Some("5".into()),
                    quote: "[12]".into(),
                    offset: Some(7),
                },
            ],
        });
    }

    /// Backward compat (R085): legacy parsers and cached fixtures produce
    /// CitationEntry without the `anchors` field. Serde default must tolerate
    /// the missing field on deserialize.
    #[test]
    fn roundtrip_citation_entry_legacy_omits_anchors() {
        let json = r#"{"raw_text":"Smith et al. 2024","doi":null,"arxiv_id":null,"title":null}"#;
        let entry: CitationEntry =
            serde_json::from_str(json).expect("legacy CitationEntry deserializes");
        assert_eq!(entry.raw_text, "Smith et al. 2024");
        assert!(entry.anchors.is_empty(), "legacy JSON has no anchors");
    }
}
