//! HTML parsed stage (M451 phase-2): html_headings grammar -> parsed/
//! article.json + context/projection.json artifacts for the CLI to
//! persist. Port of scripts/m451_textbook_import.py (cross-validated).

use da_domain::html_headings::parse_html_headings;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum HtmlParsedError {
    #[error("html parsed stage: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct HtmlParsedOutput {
    pub object_id: String,
    pub parsed_json: String,
    pub projection_json: String,
    pub sections: usize,
}

pub fn build_html_parsed(html: &str, object_id: &str) -> Result<HtmlParsedOutput, HtmlParsedError> {
    let sections = parse_html_headings(html, object_id);
    let body: String = sections
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let sha = sha256_hex(html.as_bytes());
    let title = sections
        .first()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| object_id.to_string());

    let parsed = json!({
        "schema_version": "parsed-article.v2",
        "paper_id": object_id,
        "grammar_profile": "html_headings",
        "title": title,
        "abstract_text": "",
        "body_text": body,
        "sections": sections,
        "citations": [], "formulas": [], "tables": [],
        "table_missingness": "no_table_elements",
        "formula_missingness": "no_formula_elements",
        "pdf_hash": sha,
    });
    let parsed_json =
        serde_json::to_string_pretty(&parsed).map_err(|e| HtmlParsedError::Parse(e.to_string()))?;

    let proj = json!({
        "paper_id": object_id,
        "sections": sections.iter().enumerate().map(|(i, s)| json!({
            "section_id": s.section_id, "path_id": s.path_id, "order": i,
            "level": s.level, "title": s.title, "char_count": s.char_count,
        })).collect::<Vec<_>>(),
    });
    let projection_json =
        serde_json::to_string_pretty(&proj).map_err(|e| HtmlParsedError::Parse(e.to_string()))?;
    Ok(HtmlParsedOutput {
        object_id: object_id.to_string(),
        parsed_json,
        projection_json,
        sections: sections.len(),
    })
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_parsed_and_projection() {
        let html = r#"<h1>Chapter 2</h1><p>intro</p><h2>Summary</h2><p>sum text</p>"#;
        let out = build_html_parsed(html, "ch2").unwrap();
        assert_eq!(out.sections, 2);
        let p: serde_json::Value = serde_json::from_str(&out.parsed_json).unwrap();
        assert_eq!(p["grammar_profile"], "html_headings");
        assert_eq!(p["title"], "Chapter 2");
        let secs = p["sections"].as_array().unwrap();
        assert_eq!(secs[0]["path_id"], "h1");
        let proj: serde_json::Value = serde_json::from_str(&out.projection_json).unwrap();
        assert_eq!(proj["sections"].as_array().unwrap().len(), 2);
    }
}
