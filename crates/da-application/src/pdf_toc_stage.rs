//! PDF TOC parsed stage (M451 phase-2 port #6): observations (toc +
//! page texts, reference extractor) -> parsed/article.json + projection
//! for page_outline grammars.

use da_domain::pdf_toc::{build_pdf_toc_sections, TocEntry, TocFurniture};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum PdfTocStageError {
    #[error("pdf-toc stage: observations invalid: {0}")]
    Observations(String),
    #[error("pdf-toc stage: empty toc: {0}")]
    EmptyToc(String),
}

/// Observation document (reference exporter output).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TocObservations {
    #[serde(default)]
    pub toc: Vec<TocEntry>,
    #[serde(default)]
    pub pages_text: BTreeMap<u32, String>,
    #[serde(default)]
    pub pages_margin_notes: BTreeMap<u32, String>,
    pub total_pages: u32,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pdf_sha256: Option<String>,
    #[serde(default)]
    pub furniture_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PdfTocStageOutput {
    pub object_id: String,
    pub parsed_json: String,
    pub projection_json: String,
    pub sections: usize,
    pub furniture: usize,
}

pub fn build_pdf_toc_stage(
    object_id: &str,
    obs_json: &str,
) -> Result<PdfTocStageOutput, PdfTocStageError> {
    let obs: TocObservations = serde_json::from_str(obs_json)
        .map_err(|e| PdfTocStageError::Observations(e.to_string()))?;
    if obs.toc.is_empty() {
        return Err(PdfTocStageError::EmptyToc(object_id.to_string()));
    }
    let furniture = TocFurniture::from_patterns(&obs.furniture_patterns);
    let sections = build_pdf_toc_sections(
        &obs.toc,
        &obs.pages_text,
        Some(&obs.pages_margin_notes),
        obs.total_pages,
        object_id,
        &furniture,
    );
    let body: String = sections.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("\n\n");
    let title = obs
        .title
        .clone()
        .or_else(|| sections.iter().find(|s| !s.is_furniture).map(|s| s.title.clone()))
        .unwrap_or_else(|| object_id.to_string());

    let parsed = json!({
        "schema_version": "parsed-article.v2",
        "paper_id": object_id,
        "grammar_profile": "page_outline",
        "title": title,
        "abstract_text": "",
        "body_text": body,
        "sections": sections,
        "citations": [], "formulas": [], "tables": [],
        "table_missingness": "no_table_elements",
        "formula_missingness": "no_formula_elements",
        "pdf_hash": obs.pdf_sha256.clone().unwrap_or_default(),
    });
    let parsed_json = serde_json::to_string_pretty(&parsed)
        .map_err(|e| PdfTocStageError::Observations(e.to_string()))?;

    let proj = json!({
        "paper_id": object_id,
        "sections": sections.iter().enumerate().map(|(i, s)| json!({
            "section_id": s.section_id, "path_id": format!("p{}", s.page_start),
            "order": i, "level": s.depth, "title": s.title, "char_count": s.char_count,
        })).collect::<Vec<_>>(),
    });
    let projection_json = serde_json::to_string_pretty(&proj)
        .map_err(|e| PdfTocStageError::Observations(e.to_string()))?;
    Ok(PdfTocStageOutput {
        object_id: object_id.to_string(),
        parsed_json,
        projection_json,
        sections: sections.len(),
        furniture: sections.iter().filter(|s| s.is_furniture).count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_builds_from_observations() {
        let obs = serde_json::json!({
            "toc": [
                {"level": 1, "title": "Cover", "page": 1},
                {"level": 1, "title": "Chapter 1", "page": 5},
                {"level": 2, "title": "1.1 Intro", "page": 5},
            ],
            "pages_text": {"5": "chapter body", "6": "more"},
            "total_pages": 8,
            "title": "Test Book",
            "pdf_sha256": "abc",
            "furniture_patterns": ["^Cover$"]
        });
        let out = build_pdf_toc_stage("bk", obs.to_string().as_str()).unwrap();
        assert_eq!(out.sections, 3);
        assert_eq!(out.furniture, 1);
        let p: serde_json::Value = serde_json::from_str(&out.parsed_json).unwrap();
        assert_eq!(p["grammar_profile"], "page_outline");
        assert_eq!(p["title"], "Test Book");
        assert_eq!(p["pdf_hash"], "abc");
    }

    #[test]
    fn empty_toc_is_honest_error() {
        let r = build_pdf_toc_stage("bk", r#"{"toc":[], "total_pages": 0}"#);
        assert!(matches!(r, Err(PdfTocStageError::EmptyToc(_))));
    }
}
