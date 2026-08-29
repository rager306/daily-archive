//! Equation extraction stage executor (M407 S01).
//!
//! Pure cacheable compute over the parse payload: converts every GROBID TEI
//! `ExtractedFormula` into a revision-grounded [`EquationCandidate`] via the
//! M400 domain conversion (`equation_candidate_input_from_extracted`).
//! No graph writes, no external services — equations come from the TEI
//! already inside the cached parse payload.
//!
//! Typed missingness (R087): a parse payload with no formulas and
//! `formula_missingness = None` (the parser did not attempt extraction)
//! produces an explicit no-claim payload — an empty candidate list plus the
//! `missingness=parser_not_attempted` metadata token — never a silent zero.

use std::collections::BTreeMap;

use async_trait::async_trait;
use da_domain::equation_candidate::EquationCandidate;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EQUATION_EXTRACTION_IMPL_FINGERPRINT: &str = "EquationExtractionStage@tei-v1";
pub const EQUATION_EXTRACTION_OUTPUT_SCHEMA: &str = "Vec<EquationCandidate>@serde-v1";

/// Wire artifact for the equation extraction stage: typed candidates plus
/// the parsed missingness token so an empty list is never ambiguous.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquationExtractionArtifact {
    pub candidates: Vec<EquationCandidate>,
    /// `parser_not_attempted` when the parser carried no formula structure
    /// at all; `None` when extraction ran (empty list = genuinely no
    /// formulas in the document).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missingness: Option<String>,
}

/// Canonical dependency name for the parse payload.
const DEFAULT_PARSE_DEPENDENCY: &str = "parse";

pub struct EquationExtractionStage {
    revision_vid: String,
    source_pdf_sha256: String,
    parse_dependency: String,
}

impl EquationExtractionStage {
    pub fn new(revision_vid: impl Into<String>, source_pdf_sha256: impl Into<String>) -> Self {
        Self {
            revision_vid: revision_vid.into(),
            source_pdf_sha256: source_pdf_sha256.into(),
            parse_dependency: DEFAULT_PARSE_DEPENDENCY.into(),
        }
    }

    pub fn with_parse_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.parse_dependency = dependency.into();
        self
    }

    /// Configuration fingerprint binds the exact revision identity the
    /// candidates are grounded against.
    pub fn configuration_fingerprint(&self) -> String {
        let identity = serde_json::json!({
            "revision_vid": self.revision_vid,
            "source_pdf_sha256": self.source_pdf_sha256,
        });
        let bytes = serde_json::to_vec(&identity).expect("identity serializes");
        hex::encode(Sha256::digest(&bytes))
    }
}

#[async_trait]
impl PipelineStageExecutor for EquationExtractionStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "equation_extraction".into(),
            implementation_fingerprint: EQUATION_EXTRACTION_IMPL_FINGERPRINT.into(),
            configuration_fingerprint: self.configuration_fingerprint(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: EQUATION_EXTRACTION_OUTPUT_SCHEMA.into(),
        }
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        serde_json::from_slice::<EquationExtractionArtifact>(payload)
            .map(|_| ())
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "incompatible_cache_payload".into(),
                message: error.to_string(),
            })
    }

    async fn execute(
        &self,
        request: StageExecutionRequest,
    ) -> Result<StageExecutionOutput, StageExecutionError> {
        let payload = request
            .dependency_payloads
            .get(&self.parse_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for equation extraction",
                    self.parse_dependency
                ),
            })?;
        let parsed: ParsedArticle =
            serde_json::from_slice(payload).map_err(|error| StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            })?;
        if parsed.paper_id != request.article_id {
            return Err(StageExecutionError::Terminal {
                error_class: "article_identity_mismatch".into(),
                message: format!(
                    "parsed article '{}' does not match requested '{}'",
                    parsed.paper_id, request.article_id
                ),
            });
        }
        let missingness = parsed
            .formula_missingness
            .map(|reason| format!("parser_reported:{reason:?}"));
        let mut candidates = Vec::with_capacity(parsed.formulas.len());
        let mut conversion_skipped: usize = 0;
        for formula in &parsed.formulas {
            let input = match crate::equation_materialize::equation_candidate_input_from_extracted(
                formula,
                &self.revision_vid,
                &self.source_pdf_sha256,
                &parsed.sections,
            ) {
                Ok(input) => input,
                // Typed extraction missingness (R087 / domain contract): skip
                // the ungroundable formula, count it, never kill the journey.
                Err(
                    error @ crate::equation_materialize::EquationMaterializationError::UngroundedFormula(_),
                )
                | Err(
                    error @ crate::equation_materialize::EquationMaterializationError::MalformedFormula(_, _),
                )
                | Err(
                    error @ crate::equation_materialize::EquationMaterializationError::SectionOrdinalOutOfRange { .. },
                ) => {
                    conversion_skipped += 1;
                    tracing::warn!(
                        formula_id = %formula.formula_id,
                        reason = %error,
                        event = "equation_candidate_missingness_skipped"
                    );
                    continue;
                }
                Err(error) => {
                    return Err(StageExecutionError::Terminal {
                        error_class: "equation_input_conversion".into(),
                        message: error.to_string(),
                    });
                }
            };
            let candidate =
                EquationCandidate::new(input).map_err(|error| StageExecutionError::Terminal {
                    error_class: "equation_candidate_assembly".into(),
                    message: error.to_string(),
                })?;
            candidates.push(candidate);
        }
        let missingness = match (missingness, conversion_skipped) {
            (None, 0) => None,
            (Some(token), 0) => Some(token),
            (None, skipped) => Some(format!("conversion:skipped={skipped}")),
            (Some(token), skipped) => Some(format!("{token};conversion:skipped={skipped}")),
        };
        let artifact = EquationExtractionArtifact {
            candidates,
            missingness,
        };
        let output =
            serde_json::to_vec(&artifact).map_err(|error| StageExecutionError::Terminal {
                error_class: "equation_artifact_serialization".into(),
                message: error.to_string(),
            })?;
        Ok(StageExecutionOutput {
            metadata: BTreeMap::from([
                ("equations".into(), artifact.candidates.len().to_string()),
                ("conversion_skipped".into(), conversion_skipped.to_string()),
                (
                    "missingness".into(),
                    artifact
                        .missingness
                        .clone()
                        .unwrap_or_else(|| "none".into()),
                ),
                ("side_effects".into(), "none".into()),
                ("import_eligible".into(), "false".into()),
            ]),
            payload: output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    fn parsed_with_formulas(paper_id: &str, count: usize) -> ParsedArticle {
        ParsedArticle {
            paper_id: paper_id.into(),
            title: "T".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: (0..3)
                .map(|ordinal| da_ports::parser::Section {
                    title: format!("Section {ordinal}"),
                    text: format!("text {ordinal}"),
                    level: 1,
                    path_id: Some(format!("{ordinal}")),
                    section_id: Some(format!("sec-{ordinal}")),
                    ..Default::default()
                })
                .collect(),
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: (0..count)
                .map(|i| da_ports::parser::ExtractedFormula {
                    formula_id: format!("formula_{i}"),
                    section_path_id: Some("2".into()),
                    section_ordinal: Some(2),
                    label: Some(format!("({i})")),
                    coordinates: Some("10,20,30,40;5".into()),
                    representation: da_ports::parser::FormulaRepresentation::TeXMath("L=x".into()),
                    span: Some(da_ports::parser::CharSpan { start: 1, end: 4 }),
                    source_digest: sha(7),
                    missingness: None,
                })
                .collect(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: sha(7),
            ..Default::default()
        }
    }

    fn request(paper_id: &str, parsed: &ParsedArticle) -> StageExecutionRequest {
        StageExecutionRequest {
            article_id: paper_id.into(),
            dependency_payloads: BTreeMap::from([(
                "parse".to_string(),
                serde_json::to_vec(parsed).expect("parsed serializes"),
            )]),
        }
    }

    #[test]
    fn artifact_roundtrips_through_serde() {
        // ADR-051 §4.4: the wire artifact must roundtrip losslessly.
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let parsed = parsed_with_formulas("p1", 2);
        let out = futures_executor_block_on(stage, parsed.clone());
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        let re = serde_json::to_vec(&artifact).expect("encode");
        let again: EquationExtractionArtifact = serde_json::from_slice(&re).expect("re-decode");
        assert_eq!(again.candidates.len(), 2);
    }

    fn futures_executor_block_on(
        stage: EquationExtractionStage,
        parsed: ParsedArticle,
    ) -> StageExecutionOutput {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        rt.block_on(async {
            stage
                .execute(request("p1", &parsed))
                .await
                .expect("execute")
        })
    }

    #[tokio::test]
    async fn two_formulas_become_two_grounded_candidates() {
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let parsed = parsed_with_formulas("p1", 2);
        let out = stage
            .execute(request("p1", &parsed))
            .await
            .expect("execute");
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert_eq!(artifact.candidates.len(), 2);
        assert!(artifact.missingness.is_none());
        for candidate in &artifact.candidates {
            assert_eq!(candidate.revision_vid, sha(9));
            assert_eq!(
                candidate.section_vid.len(),
                64,
                "section vid is a sha256 revision-grounded vid"
            );
        }
        assert_eq!(out.metadata.get("equations").map(String::as_str), Some("2"));
    }

    #[tokio::test]
    async fn parser_not_attempted_is_a_typed_no_claim() {
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let mut parsed = parsed_with_formulas("p1", 0);
        parsed.formula_missingness =
            Some(da_ports::parser::FormulaMissingReason::NoFormulaElements);
        let out = stage
            .execute(request("p1", &parsed))
            .await
            .expect("execute");
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert!(artifact.candidates.is_empty());
        let missingness = artifact.missingness.expect("typed missingness");
        assert!(missingness.starts_with("parser_reported:"));
        assert_eq!(
            out.metadata.get("missingness").map(String::as_str),
            Some(missingness.as_str())
        );
    }

    #[tokio::test]
    async fn ungrounded_formula_is_skipped_not_fatal() {
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let mut parsed = parsed_with_formulas("p1", 2);
        let mut stray = da_ports::parser::ExtractedFormula {
            formula_id: "formula_stray".into(),
            section_path_id: None,
            section_ordinal: None,
            label: Some("(9)".into()),
            coordinates: None,
            representation: da_ports::parser::FormulaRepresentation::TeXMath("z".into()),
            span: None,
            source_digest: sha(3),
            missingness: None,
        };
        stray.section_ordinal = None;
        stray.section_path_id = None;
        parsed.formulas.push(stray);
        let out = stage
            .execute(request("p1", &parsed))
            .await
            .expect("execute completes despite ungrounded formula");
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert_eq!(artifact.candidates.len(), 2, "grounded subset survives");
        assert_eq!(
            out.metadata.get("conversion_skipped").map(String::as_str),
            Some("1"),
            "skip must be counted, never silent"
        );
        assert_eq!(out.metadata.get("equations").map(String::as_str), Some("2"));
    }

    #[tokio::test]
    async fn all_ungrounded_completes_with_typed_token() {
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let mut parsed = parsed_with_formulas("p1", 0);
        for i in 0..2 {
            parsed.formulas.push(da_ports::parser::ExtractedFormula {
                formula_id: format!("formula_{i}"),
                section_path_id: None,
                section_ordinal: None,
                label: None,
                coordinates: None,
                representation: da_ports::parser::FormulaRepresentation::TeXMath("q".into()),
                span: None,
                source_digest: sha(4),
                missingness: None,
            });
        }
        let out = stage
            .execute(request("p1", &parsed))
            .await
            .expect("execute completes with zero grounded candidates");
        let artifact: EquationExtractionArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert!(artifact.candidates.is_empty());
        let missingness = artifact
            .missingness
            .expect("typed token, not a silent zero");
        assert!(
            missingness.contains("conversion:skipped=2"),
            "token was {missingness}"
        );
        assert_eq!(
            out.metadata.get("conversion_skipped").map(String::as_str),
            Some("2")
        );
    }

    #[tokio::test]
    async fn identity_mismatch_fails_closed() {
        let stage = EquationExtractionStage::new(sha(9), sha(7));
        let parsed = parsed_with_formulas("other", 1);
        let err = stage.execute(request("p1", &parsed)).await.unwrap_err();
        assert!(matches!(
            err,
            StageExecutionError::Terminal { error_class, .. }
                if error_class == "article_identity_mismatch"
        ));
    }

    #[test]
    fn contract_rotates_with_revision_identity() {
        let a = EquationExtractionStage::new(sha(9), sha(7));
        let b = EquationExtractionStage::new(sha(10), sha(7));
        assert_ne!(a.configuration_fingerprint(), b.configuration_fingerprint());
    }
}
