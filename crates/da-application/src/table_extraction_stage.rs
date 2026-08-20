//! Table extraction stage executor (M408 S01).
//!
//! Pure cacheable compute joining GROBID TEI tables with pdffigures2
//! detection artifacts into detection-anchored [`TableCandidate`]s with
//! typed visual missingness.
//!
//! Join policy (deterministic, fail-visible):
//! - TEI `coordinates` carry `x1,y1,x2,y2;page`; detection artifacts carry a
//!   72 DPI region plus `page_index`. A TEI table may only anchor to a
//!   `Table`-kind artifact on the **same page** whose box IoU with the TEI
//!   region is at least [`IOU_THRESHOLD`]; the best match wins and one
//!   artifact anchors at most one TEI table.
//! - Unmatched TEI tables are reported as `tei_only` rows with a reason
//!   (no artifact on page, below-threshold overlap, or unparseable
//!   coordinates) — never silently dropped and never assembled without a
//!   detection anchor.
//! - A missing/empty detection dependency is a typed `detection_absent`
//!   artifact with zero candidates.
//!
//! The visual side of the fidelity comparison stays `None` in this slice:
//! `FigureTablePayload` is an LLM product (ADR-060) and assembling it
//! without the analysis route would fabricate evidence.
//! `compare_table_fidelity(None, Some(structural))` produces the typed
//! visual-missingness report instead.

use std::collections::BTreeMap;

use async_trait::async_trait;
use da_domain::figure::DetectedVisualArtifact;
use da_domain::figure::VisualArtifactKind;
use da_domain::table_candidate::{TableCandidate, TableCandidateLineage};
use da_domain::table_fidelity::compare_table_fidelity;
use da_domain::vid::revision_section_vid;
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_stage::{
    PipelineStageExecutor, StageContract, StageExecutionError, StageExecutionOutput,
    StageExecutionRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::table_materialize::structural_table_from_extracted;

pub const TABLE_EXTRACTION_IMPL_FINGERPRINT: &str = "TableExtractionStage@tei-detection-join-v1";
pub const TABLE_EXTRACTION_OUTPUT_SCHEMA: &str = "TableCandidateArtifact@serde-v1";

/// Minimum box IoU between a TEI table region and a detection artifact on
/// the same page for the anchor to be accepted.
pub const IOU_THRESHOLD: f64 = 0.5;

const DEFAULT_PARSE_DEPENDENCY: &str = "parse";
const DEFAULT_FIGURES_DEPENDENCY: &str = "figures";

/// Typed outcome of joining one TEI table against the detection artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TableJoinRow {
    Matched {
        table_id: String,
        artifact_id: String,
        iou: f64,
    },
    TeiOnly {
        table_id: String,
        reason: String,
    },
}

/// Wire artifact for the table extraction stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCandidateArtifact {
    pub candidates: Vec<TableCandidate>,
    pub join_rows: Vec<TableJoinRow>,
    /// `detection_absent` when the figures dependency carried no artifacts;
    /// `None` when the join ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missingness: Option<String>,
}

/// Parsed TEI coordinates: `x1,y1,x2,y2;page`.
struct TeiRegion {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    page: u32,
}

fn parse_tei_coordinates(raw: &str) -> Result<TeiRegion, String> {
    let (coords, page) = raw
        .rsplit_once(';')
        .ok_or_else(|| format!("coordinates lack a page segment: {raw:?}"))?;
    let page: u32 = page
        .trim()
        .parse()
        .map_err(|error| format!("coordinates page is not an integer: {page:?} ({error})"))?;
    let parts: Vec<&str> = coords.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(format!(
            "coordinates must carry four comma-separated values: {coords:?}"
        ));
    }
    let mut values = [0f64; 4];
    for (slot, part) in values.iter_mut().zip(parts) {
        *slot = part
            .parse()
            .map_err(|error| format!("coordinate {part:?} is not a number ({error})"))?;
    }
    Ok(TeiRegion {
        x1: values[0],
        y1: values[1],
        x2: values[2],
        y2: values[3],
        page,
    })
}

fn intersection_area(a: &TeiRegion, b: &DetectedVisualArtifact) -> f64 {
    let (bx1, by1, bx2, by2) = (b.region.x1, b.region.y1, b.region.x2, b.region.y2);
    let width = (a.x2.min(bx2) - a.x1.max(bx1)).max(0.0);
    let height = (a.y2.min(by2) - a.y1.max(by1)).max(0.0);
    width * height
}

fn box_iou(a: &TeiRegion, b: &DetectedVisualArtifact) -> f64 {
    let inter = intersection_area(a, b);
    if inter <= 0.0 {
        return 0.0;
    }
    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.region.x2 - b.region.x1).max(0.0) * (b.region.y2 - b.region.y1).max(0.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

pub struct TableExtractionStage {
    revision_vid: String,
    source_pdf_sha256: String,
    parse_dependency: String,
    figures_dependency: String,
}

impl TableExtractionStage {
    pub fn new(revision_vid: impl Into<String>, source_pdf_sha256: impl Into<String>) -> Self {
        Self {
            revision_vid: revision_vid.into(),
            source_pdf_sha256: source_pdf_sha256.into(),
            parse_dependency: DEFAULT_PARSE_DEPENDENCY.into(),
            figures_dependency: DEFAULT_FIGURES_DEPENDENCY.into(),
        }
    }

    pub fn with_dependencies(
        mut self,
        parse_dependency: impl Into<String>,
        figures_dependency: impl Into<String>,
    ) -> Self {
        self.parse_dependency = parse_dependency.into();
        self.figures_dependency = figures_dependency.into();
        self
    }

    pub fn configuration_fingerprint(&self) -> String {
        let identity = serde_json::json!({
            "revision_vid": self.revision_vid,
            "source_pdf_sha256": self.source_pdf_sha256,
            "iou_threshold": IOU_THRESHOLD,
        });
        let bytes = serde_json::to_vec(&identity).expect("identity serializes");
        hex::encode(Sha256::digest(&bytes))
    }
}

#[async_trait]
impl PipelineStageExecutor for TableExtractionStage {
    fn contract(&self) -> StageContract {
        StageContract {
            stage_id: "table_fidelity".into(),
            implementation_fingerprint: TABLE_EXTRACTION_IMPL_FINGERPRINT.into(),
            configuration_fingerprint: self.configuration_fingerprint(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: TABLE_EXTRACTION_OUTPUT_SCHEMA.into(),
        }
    }

    fn validate_cached_payload(&self, payload: &[u8]) -> Result<(), StageExecutionError> {
        serde_json::from_slice::<TableCandidateArtifact>(payload)
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
        let parse_payload = request
            .dependency_payloads
            .get(&self.parse_dependency)
            .ok_or_else(|| StageExecutionError::Terminal {
                error_class: "missing_parsed_dependency".into(),
                message: format!(
                    "dependency '{}' is required for table extraction",
                    self.parse_dependency
                ),
            })?;
        let parsed: ParsedArticle = serde_json::from_slice(parse_payload).map_err(|error| {
            StageExecutionError::Terminal {
                error_class: "parsed_article_deserialization".into(),
                message: error.to_string(),
            }
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

        let artifacts: Vec<DetectedVisualArtifact> =
            match request.dependency_payloads.get(&self.figures_dependency) {
                None => Vec::new(),
                Some(payload) => serde_json::from_slice(payload).map_err(|error| {
                    StageExecutionError::Terminal {
                        error_class: "detection_artifacts_deserialization".into(),
                        message: error.to_string(),
                    }
                })?,
            };
        if artifacts.is_empty() {
            let artifact = TableCandidateArtifact {
                candidates: Vec::new(),
                join_rows: Vec::new(),
                missingness: Some("detection_absent".into()),
            };
            return encode(artifact, "detection_absent");
        }

        let tables_only: Vec<&DetectedVisualArtifact> = artifacts
            .iter()
            .filter(|artifact| artifact.kind == VisualArtifactKind::Table)
            .collect();
        let mut anchored: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        let mut candidates = Vec::new();
        let mut join_rows = Vec::new();
        for table in &parsed.tables {
            let Some(raw_coords) = table.coordinates.as_deref() else {
                join_rows.push(TableJoinRow::TeiOnly {
                    table_id: table.table_id.clone(),
                    reason: "tei_coordinates_absent".into(),
                });
                continue;
            };
            let region = match parse_tei_coordinates(raw_coords) {
                Ok(region) => region,
                Err(reason) => {
                    join_rows.push(TableJoinRow::TeiOnly {
                        table_id: table.table_id.clone(),
                        reason,
                    });
                    continue;
                }
            };
            // Best same-page Table artifact by IoU, each anchored at most once.
            let mut best: Option<(f64, &DetectedVisualArtifact)> = None;
            for artifact in &tables_only {
                if artifact.page_index != region.page || anchored.contains(&artifact.artifact_id) {
                    continue;
                }
                let iou = box_iou(&region, artifact);
                if iou >= IOU_THRESHOLD && best.is_none_or(|(best_iou, _)| iou > best_iou) {
                    best = Some((iou, artifact));
                }
            }
            let Some((iou, artifact)) = best else {
                join_rows.push(TableJoinRow::TeiOnly {
                    table_id: table.table_id.clone(),
                    reason: format!(
                        "no_table_artifact_on_page_{}_at_iou_{}",
                        region.page, IOU_THRESHOLD
                    ),
                });
                continue;
            };
            let Some(section_ordinal) = table.section_ordinal else {
                join_rows.push(TableJoinRow::TeiOnly {
                    table_id: table.table_id.clone(),
                    reason: "tei_section_ordinal_absent".into(),
                });
                continue;
            };
            let section_vid = revision_section_vid(&self.revision_vid, section_ordinal)
                .map_err(|error| StageExecutionError::Terminal {
                    error_class: "table_section_grounding".into(),
                    message: error,
                })?
                .to_string();
            let section_title = parsed
                .sections
                .get(section_ordinal)
                .map(|section| section.title.clone())
                .unwrap_or_default();
            let lineage = TableCandidateLineage {
                revision_vid: self.revision_vid.clone(),
                artifact_id: artifact.artifact_id.clone(),
                detector_name: artifact.detector_name.clone(),
                detector_fingerprint: artifact.detector_fingerprint.clone(),
                image_sha256: artifact.asset_sha256.clone(),
                asset_path: artifact.asset_path.clone(),
                page_index: artifact.page_index,
                section_vid: section_vid.clone(),
                section_title,
                section_ordinal: section_ordinal as u32,
                caption: artifact.caption.clone(),
                source_pdf_sha256: self.source_pdf_sha256.clone(),
            };
            let structural = structural_table_from_extracted(table);
            let report = compare_table_fidelity(None, Some(&structural)).map_err(|error| {
                StageExecutionError::Terminal {
                    error_class: "table_fidelity_comparison".into(),
                    message: error.to_string(),
                }
            })?;
            let candidate = TableCandidate::assemble(
                &lineage,
                // The visual side is an LLM product (ADR-060); typed None
                // until the analysis route joins the journey.
                None,
                &structural,
                &report,
                table.coordinates.clone(),
            )
            .map_err(|error| StageExecutionError::Terminal {
                error_class: "table_candidate_assembly".into(),
                message: error.to_string(),
            })?;
            anchored.insert(artifact.artifact_id.clone());
            join_rows.push(TableJoinRow::Matched {
                table_id: table.table_id.clone(),
                artifact_id: artifact.artifact_id.clone(),
                iou,
            });
            candidates.push(candidate);
        }

        let artifact = TableCandidateArtifact {
            candidates,
            join_rows,
            missingness: None,
        };
        let matched = artifact
            .join_rows
            .iter()
            .filter(|row| matches!(row, TableJoinRow::Matched { .. }))
            .count();
        let metadata = format!("matched={matched}");
        encode(artifact, &metadata)
    }
}

fn encode(
    artifact: TableCandidateArtifact,
    metadata_summary: &str,
) -> Result<StageExecutionOutput, StageExecutionError> {
    let payload = serde_json::to_vec(&artifact).map_err(|error| StageExecutionError::Terminal {
        error_class: "table_artifact_serialization".into(),
        message: error.to_string(),
    })?;
    let matched = artifact
        .join_rows
        .iter()
        .filter(|row| matches!(row, TableJoinRow::Matched { .. }))
        .count();
    Ok(StageExecutionOutput {
        metadata: BTreeMap::from([
            ("tables".into(), artifact.candidates.len().to_string()),
            ("matched".into(), matched.to_string()),
            ("join_rows".into(), artifact.join_rows.len().to_string()),
            ("summary".into(), metadata_summary.to_string()),
            ("side_effects".into(), "none".into()),
            ("import_eligible".into(), "false".into()),
        ]),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::figure::BoundingBox72Dpi;
    use da_ports::parser::{TableCell, TableMissingReason};

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    fn section(ordinal: usize) -> da_ports::parser::Section {
        da_ports::parser::Section {
            title: format!("Section {ordinal}"),
            text: format!("text {ordinal}"),
            level: 1,
            path_id: Some(format!("{ordinal}")),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        }
    }

    fn parsed_with_tables(
        paper_id: &str,
        tables: Vec<da_ports::parser::ExtractedTable>,
    ) -> ParsedArticle {
        ParsedArticle {
            paper_id: paper_id.into(),
            title: "T".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: (0..3).map(section).collect(),
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables,
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            page_text_missingness: None,
            pdf_hash: sha(7),
        }
    }

    fn tei_table(
        table_id: &str,
        coordinates: Option<&str>,
        ordinal: usize,
    ) -> da_ports::parser::ExtractedTable {
        da_ports::parser::ExtractedTable {
            table_id: table_id.into(),
            section_path_id: Some(format!("{ordinal}")),
            section_ordinal: Some(ordinal),
            caption: "Table 1 :".into(),
            coordinates: coordinates.map(str::to_string),
            cells: vec![TableCell {
                row: 0,
                column: 0,
                colspan: 1,
                rowspan: 1,
                text: "v".into(),
                coordinates: None,
                span: None,
                digest: sha(5),
            }],
            source_digest: sha(3),
            missingness: None,
        }
    }

    fn table_artifact(
        id: &str,
        page: u32,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) -> DetectedVisualArtifact {
        DetectedVisualArtifact {
            artifact_id: id.into(),
            kind: VisualArtifactKind::Table,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            source_pdf_sha256: sha(1),
            page_index: page,
            region: BoundingBox72Dpi { x1, y1, x2, y2 },
            caption_region: BoundingBox72Dpi {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            caption: "Table 1: results.".into(),
            image_text: vec![],
            render_dpi: 72,
            asset_path: format!("figures/{id}.png"),
            asset_sha256: sha(9),
            metadata_sha256: sha(4),
        }
    }

    fn request(
        paper_id: &str,
        parsed: &ParsedArticle,
        artifacts: Option<Vec<DetectedVisualArtifact>>,
    ) -> StageExecutionRequest {
        let mut deps = BTreeMap::from([(
            "parse".to_string(),
            serde_json::to_vec(parsed).expect("parsed serializes"),
        )]);
        if let Some(artifacts) = artifacts {
            deps.insert(
                "figures".to_string(),
                serde_json::to_vec(&artifacts).expect("artifacts serializes"),
            );
        }
        StageExecutionRequest {
            article_id: paper_id.into(),
            dependency_payloads: deps,
        }
    }

    fn stage() -> TableExtractionStage {
        TableExtractionStage::new(sha(9), sha(1))
    }

    #[tokio::test]
    async fn matched_same_page_join_assembles_one_candidate() {
        let parsed = parsed_with_tables("p1", vec![tei_table("tab_0", Some("10,20,500,300;3"), 1)]);
        let out = stage()
            .execute(request(
                "p1",
                &parsed,
                Some(vec![table_artifact("Table1", 3, 10.0, 20.0, 500.0, 300.0)]),
            ))
            .await
            .expect("execute");
        let artifact: TableCandidateArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert_eq!(artifact.candidates.len(), 1);
        assert!(artifact.missingness.is_none());
        match &artifact.join_rows[0] {
            TableJoinRow::Matched {
                artifact_id, iou, ..
            } => {
                assert_eq!(artifact_id, "Table1");
                assert!(*iou > 0.99, "identical boxes: iou={iou}");
            }
            other => panic!("expected matched row, got {other:?}"),
        }
        assert_eq!(artifact.candidates[0].artifact_id, "Table1");
    }

    #[tokio::test]
    async fn below_threshold_and_other_page_are_tei_only() {
        let parsed = parsed_with_tables(
            "p1",
            vec![
                tei_table("tab_0", Some("10,20,500,300;3"), 1),
                tei_table("tab_1", Some("10,20,500,300;9"), 1),
            ],
        );
        let out = stage()
            .execute(request(
                "p1",
                &parsed,
                Some(vec![table_artifact(
                    "Table1", 3, 400.0, 250.0, 900.0, 600.0,
                )]),
            ))
            .await
            .expect("execute");
        let artifact: TableCandidateArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert_eq!(artifact.candidates.len(), 0, "no anchor qualifies");
        for row in &artifact.join_rows {
            match row {
                TableJoinRow::TeiOnly { reason, .. } => {
                    assert!(
                        reason.contains("iou") || reason.contains("page"),
                        "{reason}"
                    )
                }
                other => panic!("expected tei_only rows, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn each_artifact_anchors_at_most_one_table() {
        let parsed = parsed_with_tables(
            "p1",
            vec![
                tei_table("tab_0", Some("10,20,500,300;3"), 1),
                tei_table("tab_1", Some("12,22,498,298;3"), 2),
            ],
        );
        let out = stage()
            .execute(request(
                "p1",
                &parsed,
                Some(vec![table_artifact("Table1", 3, 10.0, 20.0, 500.0, 300.0)]),
            ))
            .await
            .expect("execute");
        let artifact: TableCandidateArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert_eq!(artifact.candidates.len(), 1, "one anchor, one candidate");
        assert_eq!(artifact.join_rows.len(), 2);
    }

    #[tokio::test]
    async fn absent_figures_dependency_is_typed_detection_absent() {
        let parsed = parsed_with_tables("p1", vec![tei_table("tab_0", Some("10,20,500,300;3"), 1)]);
        let out = stage()
            .execute(request("p1", &parsed, None))
            .await
            .expect("execute");
        let artifact: TableCandidateArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert!(artifact.candidates.is_empty());
        assert_eq!(artifact.missingness.as_deref(), Some("detection_absent"));
        assert_eq!(
            out.metadata.get("summary").map(String::as_str),
            Some("detection_absent")
        );
        let _ = TableMissingReason::NoTableElements; // import guard
    }

    #[tokio::test]
    async fn malformed_coordinates_are_typed_tei_only() {
        let parsed = parsed_with_tables("p1", vec![tei_table("tab_0", Some("garbage"), 1)]);
        let out = stage()
            .execute(request(
                "p1",
                &parsed,
                Some(vec![table_artifact("Table1", 3, 10.0, 20.0, 500.0, 300.0)]),
            ))
            .await
            .expect("execute");
        let artifact: TableCandidateArtifact =
            serde_json::from_slice(&out.payload).expect("decode");
        assert!(artifact.candidates.is_empty());
        match &artifact.join_rows[0] {
            TableJoinRow::TeiOnly { reason, .. } => assert!(reason.contains("coordinates")),
            other => panic!("expected tei_only, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn identity_mismatch_fails_closed() {
        let parsed = parsed_with_tables("other", vec![]);
        let err = stage()
            .execute(request("p1", &parsed, None))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            StageExecutionError::Terminal { error_class, .. }
                if error_class == "article_identity_mismatch"
        ));
    }
}
