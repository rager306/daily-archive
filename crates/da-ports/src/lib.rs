//! da-ports — trait definitions (ports in hexagonal architecture).
//!
//! ADR-037 §2: Ports layer depends on Domain only.
//! These traits define the boundaries between our application and external systems.

pub mod acquisition;
pub mod agent_memory;
pub mod agent_read_plane;
pub mod algorithms;
pub mod assurance;
pub mod cache_payload;
pub mod catalog;
pub mod clock;
pub mod embedder;
pub mod evidence_store;
pub mod extractor;
pub mod figure_detection;
pub mod fsm_policy;
pub mod graph_store;
pub mod llm_client;
pub mod multimodal_llm;
pub mod negative_memory;
pub mod openalex;
pub mod parse_learning;
pub mod parser;
pub mod path_config;
pub mod paths;
pub mod region_text;
pub mod schema_objects;
pub use region_text::{RegionTextOutcome, RegionTextPort};
pub mod pipeline_cache;
pub mod pipeline_stage;
pub mod promotion;
pub mod prompt_execution;
pub mod tex_source;

pub use acquisition::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, MockSourceAcquisition,
    ResolvedArticleRef, SourceAcquisitionPort,
};
pub use agent_read_plane::{
    AgentReadPlane, AgentReadPlaneError, AgentReadPlaneResult, AgentSynthesizer,
    snapshot_to_graph_identity,
};
pub use algorithms::GraphAlgorithms;
pub use assurance::{AssuranceReportStore, AssuranceStoreError};
pub use cache_payload::{
    CachePayloadError, EXTRACTED_ENTITY_SCHEMA_ID, decode_extracted_entities,
    extracted_entity_json_schema, extracted_entity_output_schema_fingerprint,
};
pub use catalog::{
    CatalogInventoryError, CatalogInventoryPort, CatalogInventorySnapshot, CatalogMutationPort,
    CatalogPdfLocator, CatalogWriteDisposition,
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

pub use parse_learning::{
    CVF_SCHEMA_VERSION, CandidateReceipt, CheckpointReceipt, CriticReceipt, CvfEvent,
    ExperienceReceipt, LEARNING_CONTRACT_VERSION, LearningContractError, LearningLineage,
    LearningStorageConfig, MAX_LEARNING_CAPACITY, MAX_RECEIPT_BYTES, PromotionState,
    RVF_SCHEMA_VERSION, Sha256Digest, SplitIdentity, reject_raw_content,
};

pub use parser::{
    CalloutUnresolvedReason, CharSpan, CitationAnchor, CitationEntry, DeclaredArxivIdentity,
    DeclaredIdMissingReason, ExtractedFigure, ExtractedFormula, ExtractedTable,
    FigureGraphicMissingReason, FormulaMissingReason, FormulaRepresentation, HeaderAuthor,
    PageTextMissing, ParserPort, Section, TableCell, TableMissingReason, VisualCallout,
    VisualCalloutKind, page_text_missingness_from,
};
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
            tables: vec![],
            table_missingness: Some(TableMissingReason::NoTableElements),
            formulas: vec![],
            formula_missingness: None,
            page_text_missingness: None,
            ..Default::default()
        });
    }

    /// M400 S02 T01: a ParsedArticle carrying extracted TEI tables survives
    /// roundtrip with table_id, section identity, coordinates, cells, spans
    /// and digests intact.
    #[test]
    fn roundtrip_parsed_article_with_tables() {
        roundtrip_json(&ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "abc123".into(),
            tables: vec![ExtractedTable {
                table_id: "tab_0".into(),
                section_path_id: Some("4".into()),
                section_ordinal: Some(3),
                caption: "Table 1 :".into(),
                coordinates: Some("90,91,428,331;1".into()),
                cells: vec![TableCell {
                    row: 0,
                    column: 0,
                    colspan: 2,
                    rowspan: 1,
                    text: "Model".into(),
                    coordinates: None,
                    span: Some(CharSpan { start: 12, end: 17 }),
                    digest: "a".repeat(64),
                }],
                source_digest: "b".repeat(64),
                missingness: None,
            }],
            table_missingness: None,
            formulas: vec![],
            formula_missingness: None,
            page_text_missingness: None,
            ..Default::default()
        });
    }

    /// M400 S02 T01: ExtractedTable + TableCell + TableMissingReason wire
    /// formats are stable (serde tag snake_case for the missingness enum).
    #[test]
    fn roundtrip_table_types() {
        roundtrip_json(&TableCell {
            row: 2,
            column: 1,
            colspan: 1,
            rowspan: 3,
            text: "42.33".into(),
            coordinates: Some("10,20,30,40;2".into()),
            span: None,
            digest: "c".repeat(64),
        });
        roundtrip_json(&TableMissingReason::NoTableElements);
        roundtrip_json(&TableMissingReason::Malformed("zero rows".into()));
        let json = serde_json::to_string(&TableMissingReason::NoTableElements).unwrap();
        assert_eq!(json, "\"no_table_elements\"");
    }

    /// M400 S03 T01 (R085): legacy ParsedArticle JSON without `tables` /
    /// `table_missingness` / `formulas` / `formula_missingness` must
    /// deserialize with typed defaults — empty Vecs and None missingness — so
    /// cached fixtures and older payloads stay decodable.
    #[test]
    fn roundtrip_parsed_article_legacy_omits_tables() {
        let json = r#"{"paper_id":"2507.19457","title":"GEPA","abstract_text":"a","body_text":"b","sections":[],"citations":[],"layout_json":null,"tei_xml":null,"pdf_hash":"abc123"}"#;
        let article: ParsedArticle =
            serde_json::from_str(json).expect("legacy ParsedArticle deserializes");
        assert_eq!(article.paper_id, "2507.19457");
        assert!(article.tables.is_empty(), "legacy JSON has no tables");
        assert!(
            article.table_missingness.is_none(),
            "legacy payload does not claim typed missingness"
        );
        assert!(article.formulas.is_empty(), "legacy JSON has no formulas");
        assert!(
            article.formula_missingness.is_none(),
            "legacy payload does not claim formula typed missingness"
        );
        assert!(
            article.page_text_missingness.is_none(),
            "legacy payload does not claim page-text missingness"
        );
    }

    /// M464-z84in2 S07/T01 (R085): legacy ParsedArticle JSON predating the
    /// S07 P0 families decodes with typed defaults — no invented identity,
    /// no missingness claims, empty header/figure/callout families.
    #[test]
    fn roundtrip_parsed_article_legacy_omits_s07_families() {
        let json = r#"{"paper_id":"2507.19457","title":"GEPA","abstract_text":"a","body_text":"b","sections":[],"citations":[],"layout_json":null,"tei_xml":null,"pdf_hash":"abc123","tables":[],"table_missingness":null,"formulas":[],"formula_missingness":null,"page_text_missingness":null}"#;
        let article: ParsedArticle =
            serde_json::from_str(json).expect("pre-S07 ParsedArticle deserializes");
        assert!(article.declared_arxiv_id.is_none());
        assert!(article.declared_id_missingness.is_none());
        assert!(article.header_authors.is_empty());
        assert!(article.figures.is_empty());
        assert!(article.visual_callouts.is_empty());
    }

    /// M464-z84in2 S07/T01: the additive P0 wire families survive roundtrip
    /// with identity, author order (Unicode + incomplete names), figure
    /// coordinates, graphic missingness and unresolved callout state intact.
    #[test]
    fn roundtrip_parsed_article_with_s07_families() {
        roundtrip_json(&ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "abc123".into(),
            tables: vec![],
            table_missingness: None,
            formulas: vec![],
            formula_missingness: None,
            page_text_missingness: None,
            declared_arxiv_id: Some(DeclaredArxivIdentity {
                raw: "arXiv:2507.19457v2[cs.CL]".into(),
                bare_id: "2507.19457".into(),
                revision: Some("v2".into()),
                source_digest: "a".repeat(64),
            }),
            declared_id_missingness: None,
            header_authors: vec![
                HeaderAuthor {
                    name: "Ozan \u{130}rsoy".into(),
                    forename: Some("Ozan".into()),
                    surname: Some("\u{130}rsoy".into()),
                    source_digest: "b".repeat(64),
                },
                HeaderAuthor {
                    name: "Yu".into(),
                    forename: None,
                    surname: None,
                    source_digest: "c".repeat(64),
                },
            ],
            figures: vec![ExtractedFigure {
                figure_id: "fig_0".into(),
                label: Some("Figure 1".into()),
                caption: Some("Overview".into()),
                coordinates: Some("90,91.80,190.43,428.38,331.74".into()),
                graphic_missingness: None,
                source_digest: "d".repeat(64),
            }],
            visual_callouts: vec![VisualCallout {
                kind: VisualCalloutKind::Figure,
                target: None,
                unresolved: Some(CalloutUnresolvedReason::NoTarget),
                quote: "[12]".into(),
                offset: Some(41),
                section_path_id: Some("3.2".into()),
                section_ordinal: Some(1),
                source_digest: "e".repeat(64),
            }],
        });
    }

    /// M464-z84in2 S07/T01: the closed S07 enum wire tags are stable
    /// snake_case strings, and a revision-less declared identity roundtrips
    /// with `revision: null` — never rewritten to a guessed v1.
    #[test]
    fn roundtrip_s07_wire_tags_and_revision_honesty() {
        roundtrip_json(&DeclaredArxivIdentity {
            raw: "2507.19457".into(),
            bare_id: "2507.19457".into(),
            revision: None,
            source_digest: "a".repeat(64),
        });
        roundtrip_json(&DeclaredIdMissingReason::NoIdno);
        roundtrip_json(&DeclaredIdMissingReason::Malformed(
            "not an arxiv id".into(),
        ));
        roundtrip_json(&FigureGraphicMissingReason::NoGraphicChild);
        roundtrip_json(&FigureGraphicMissingReason::GraphicWithoutCoordinates);
        roundtrip_json(&FigureGraphicMissingReason::Malformed("bad coords".into()));
        roundtrip_json(&CalloutUnresolvedReason::NoTarget);
        roundtrip_json(&CalloutUnresolvedReason::TargetNotInInventory);
        roundtrip_json(&VisualCalloutKind::Figure);
        roundtrip_json(&VisualCalloutKind::Table);

        assert_eq!(
            serde_json::to_string(&DeclaredIdMissingReason::NoIdno).unwrap(),
            "\"no_idno\""
        );
        assert_eq!(
            serde_json::to_string(&FigureGraphicMissingReason::GraphicWithoutCoordinates).unwrap(),
            "\"graphic_without_coordinates\""
        );
        assert_eq!(
            serde_json::to_string(&CalloutUnresolvedReason::TargetNotInInventory).unwrap(),
            "\"target_not_in_inventory\""
        );
        assert_eq!(
            serde_json::to_string(&VisualCalloutKind::Figure).unwrap(),
            "\"figure\""
        );

        let identity = DeclaredArxivIdentity {
            raw: "2507.19457".into(),
            bare_id: "2507.19457".into(),
            revision: None,
            source_digest: "a".repeat(64),
        };
        let json = serde_json::to_string(&identity).unwrap();
        assert!(
            json.contains("\"revision\":null"),
            "wire keeps null revision"
        );
        let back: DeclaredArxivIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(back.revision, None, "revision is never guessed");
    }

    /// M400 S03 T01: a ParsedArticle carrying extracted TEI formulas survives
    /// roundtrip with formula_id, label, coordinates, representation, section
    /// identity, span and digest intact.
    #[test]
    fn roundtrip_parsed_article_with_formulas() {
        roundtrip_json(&ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: vec![],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            pdf_hash: "abc123".into(),
            tables: vec![],
            table_missingness: None,
            formulas: vec![ExtractedFormula {
                formula_id: "formula_0".into(),
                section_path_id: Some("2".into()),
                section_ordinal: Some(1),
                label: Some("(1)".into()),
                coordinates: Some("90,120,428,140;1".into()),
                representation: FormulaRepresentation::PlainText("x = 1".into()),
                span: Some(CharSpan { start: 41, end: 46 }),
                source_digest: "c".repeat(64),
                missingness: None,
            }],
            formula_missingness: None,
            page_text_missingness: None,
            ..Default::default()
        });
    }

    /// M400 S03 T01: ExtractedFormula + FormulaRepresentation +
    /// FormulaMissingReason wire formats are stable (serde tag snake_case).
    #[test]
    fn roundtrip_formula_types() {
        roundtrip_json(&ExtractedFormula {
            formula_id: "formula_3".into(),
            section_path_id: None,
            section_ordinal: None,
            label: None,
            coordinates: None,
            representation: FormulaRepresentation::MathML(
                "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi></math>".into(),
            ),
            span: None,
            source_digest: "d".repeat(64),
            missingness: None,
        });
        roundtrip_json(&FormulaRepresentation::TeXMath("x = \\gamma".into()));
        roundtrip_json(&FormulaRepresentation::PlainText("y = mx + b".into()));
        roundtrip_json(&FormulaMissingReason::NoFormulaElements);
        roundtrip_json(&FormulaMissingReason::Malformed(
            "empty formula content".into(),
        ));
        let json = serde_json::to_string(&FormulaMissingReason::NoFormulaElements).unwrap();
        assert_eq!(json, "\"no_formula_elements\"");
        let repr = serde_json::to_string(&FormulaRepresentation::MathML("<m/>".into())).unwrap();
        assert_eq!(repr, "{\"math_ml\":\"<m/>\"}");
        assert_eq!(FormulaRepresentation::PlainText("t".into()).as_str(), "t");
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
