//! Structural elements stage (M451 phase-2 port from
//! scripts/m451_monograph_import.py): LaTeX-environment kinds detected
//! in section text (Theorem/Definition/Exercise/…) — DATA patterns,
//! precompiled. Pure logic; feeds Layer J (definition_source), agent
//! anchors, completeness signals.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// One detected structural element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuralElement {
    pub section_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbering: Option<String>,
    pub statement_chars: usize,
    #[serde(default)]
    pub definition_source: bool,
}

/// One kind spec from data/ontology/structural_elements_detectors.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindSpec {
    pub pattern: String,
    #[serde(default)]
    pub kind_family: String,
    #[serde(default)]
    pub definition_source: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StructuralElementsError {
    #[error("structural detectors: YAML parse failed: {0}")]
    Parse(String),
    #[error("structural detectors: kind {kind:?} pattern invalid: {err}")]
    BadPattern { kind: String, err: String },
}

/// Parsed detector registry: kind -> (regex, definition_source).
#[derive(Debug, Clone)]
pub struct StructuralDetectors {
    pub kinds: Vec<(String, Regex, bool)>,
    numbering: Option<Regex>,
}

impl StructuralDetectors {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, StructuralElementsError> {
        let v: serde_yaml::Value = serde_yaml::from_str(yaml)
            .map_err(|e| StructuralElementsError::Parse(e.to_string()))?;
        let numbering_pat = v
            .get("extraction")
            .and_then(|e| e.get("numbering_capture"))
            .and_then(|x| x.as_str())
            .map(|p| {
                Regex::new(p).map_err(|e| StructuralElementsError::BadPattern {
                    kind: "numbering".into(),
                    err: e.to_string(),
                })
            })
            .transpose()?;
        let mut kinds = Vec::new();
        if let Some(map) = v.get("kinds").and_then(|k| k.as_mapping()) {
            for (k, spec) in map {
                let Some(kind) = k.as_str() else { continue };
                let pat = spec.get("pattern").and_then(|p| p.as_str()).unwrap_or("");
                if pat.is_empty() {
                    continue;
                }
                let re = Regex::new(pat).map_err(|e| StructuralElementsError::BadPattern {
                    kind: kind.to_string(),
                    err: e.to_string(),
                })?;
                let def_src = spec
                    .get("definition_source")
                    .and_then(|d| d.as_bool())
                    .unwrap_or(false);
                kinds.push((kind.to_string(), re, def_src));
            }
        }
        Ok(Self { kinds, numbering: numbering_pat })
    }

    /// Detect elements in one section's text (line-anchored, as the
    /// reference: only line starts matter). Returns records in order.
    pub fn detect_in_text(
        &self,
        section_id: &str,
        page: Option<u32>,
        text: &str,
    ) -> Vec<StructuralElement> {
        let mut out = Vec::new();
        for line in text.lines() {
            let ls = line.trim();
            if ls.is_empty() {
                continue;
            }
            for (kind, re, def_src) in &self.kinds {
                if re.is_match(ls) {
                    out.push(StructuralElement {
                        section_id: section_id.to_string(),
                        page,
                        kind: kind.clone(),
                        numbering: self.numbering.as_ref().and_then(|n| n.find(ls)).map(|m| m.as_str().to_string()),
                        statement_chars: ls.chars().count(),
                        definition_source: *def_src,
                    });
                    break; // first matching kind wins (registry order)
                }
            }
        }
        out
    }
}

/// Input section slice (from parsed/article.json).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionInput {
    pub section_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_start: Option<u32>,
    #[serde(default)]
    pub text: String,
    /// skip furniture sections (role comes from the navigation stage)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_furniture: bool,
}

/// Detect across all sections.
pub fn detect_all(
    detectors: &StructuralDetectors,
    sections: &[SectionInput],
) -> Vec<StructuralElement> {
    sections
        .iter()
        .filter(|s| !s.is_furniture)
        .flat_map(|s| detectors.detect_in_text(&s.section_id, s.page_start, &s.text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "kinds:\n  theorem: {pattern: '^Theorem \\d+\\.\\d+', definition_source: true}\n  exercise: {pattern: '^Exercise \\d+\\.\\d+'}\nextraction:\n  numbering_capture: '\\d+\\.\\d+'\n";

    #[test]
    fn parses_registry_and_compiles() {
        let d = StructuralDetectors::from_yaml_str(YAML).unwrap();
        assert_eq!(d.kinds.len(), 2);
        assert!(d.numbering.is_some());
    }

    #[test]
    fn detects_line_anchored_kinds_with_numbering() {
        let d = StructuralDetectors::from_yaml_str(YAML).unwrap();
        let text = "Some intro text.\nTheorem 1.3. Let f be continuous.\nThen more.\nExercise 2.1. Prove it.";
        let els = d.detect_in_text("s1", Some(7), text);
        assert_eq!(els.len(), 2);
        assert_eq!(els[0].kind, "theorem");
        assert_eq!(els[0].numbering.as_deref(), Some("1.3"));
        assert_eq!(els[0].page, Some(7));
        assert!(els[0].definition_source);
        assert_eq!(els[1].kind, "exercise");
        assert!(!els[1].definition_source);
    }

    #[test]
    fn mid_sentence_mention_is_not_detected() {
        let d = StructuralDetectors::from_yaml_str(YAML).unwrap();
        let els = d.detect_in_text("s", None, "we invoke Theorem 1.1 here");
        assert!(els.is_empty()); // line-anchored only
    }

    #[test]
    fn detect_all_skips_furniture() {
        let d = StructuralDetectors::from_yaml_str(YAML).unwrap();
        let secs = vec![
            SectionInput { section_id: "a".into(), page_start: None, text: "Theorem 2.2. X".into(), is_furniture: false },
            SectionInput { section_id: "b".into(), page_start: None, text: "Theorem 9.9. Y".into(), is_furniture: true },
        ];
        let els = detect_all(&d, &secs);
        assert_eq!(els.len(), 1);
        assert_eq!(els[0].section_id, "a");
    }
}
