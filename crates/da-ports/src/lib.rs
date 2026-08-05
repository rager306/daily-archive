//! da-ports — trait definitions (ports in hexagonal architecture).
//!
//! ADR-037 §2: Ports layer depends on Domain only.
//! These traits define the boundaries between our application and external systems.

pub mod algorithms;
pub mod clock;
pub mod embedder;
pub mod evidence_store;
pub mod extractor;
pub mod graph_store;
pub mod llm_client;
pub mod openalex;
pub mod parser;

pub use algorithms::GraphAlgorithms;
pub use clock::{Clock, FixedClock, SystemClock};
pub use embedder::Embedder;
pub use evidence_store::EvidenceStore;
pub use extractor::{ExtractedEntity, Extractor};
pub use graph_store::{DirectGraphStore, GraphStore};
pub use llm_client::{ChatMessage, ChatOptions, ChatResponse, LLMClient};
pub use openalex::{OpenAlexAuthor, OpenAlexClient, OpenAlexConcept, OpenAlexTopic, OpenAlexWork};
pub mod pdf_classification;
pub use pdf_classification::{ClassificationError, ClassificationResult, NullClassifier, PdfClassificationPort};

pub use parser::ParserPort;

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
        });
    }

    #[test]
    fn roundtrip_citation_entry() {
        roundtrip_json(&CitationEntry {
            raw_text: "Smith et al. 2024".into(),
            doi: Some("10.1234/abc".into()),
            arxiv_id: None,
            title: Some("Title".into()),
        });
    }
}
