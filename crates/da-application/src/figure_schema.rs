//! Stage-2 figure schema selection (ADR-060).
//!
//! Resolves DATA-driven per-type JSON schemas for VLM structured output.
//! Does not call models; selection is the seam between Stage1 classify and Stage2 extract.

use da_domain::{
    FigureAnalysisError, FigureSchemaRegistry, FigureSchemaSelection, FigureTypeClassification,
    VisualAnalysisType,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FigureSchemaError {
    #[error(transparent)]
    Domain(#[from] FigureAnalysisError),
}

/// Application facade over bundled FigureSchemaRegistry.
pub struct FigureSchemaService {
    registry: FigureSchemaRegistry,
}

impl FigureSchemaService {
    pub fn bundled() -> Result<Self, FigureSchemaError> {
        Ok(Self {
            registry: FigureSchemaRegistry::bundled()?,
        })
    }

    pub fn new(registry: FigureSchemaRegistry) -> Self {
        Self { registry }
    }

    pub fn registry_schema_version(&self) -> &str {
        &self.registry.schema_version
    }

    pub fn select_for_type(
        &self,
        visual_type: VisualAnalysisType,
    ) -> Result<FigureSchemaSelection, FigureSchemaError> {
        Ok(self.registry.select_for(visual_type)?)
    }

    pub fn select_for_classification(
        &self,
        classification: &FigureTypeClassification,
    ) -> Result<FigureSchemaSelection, FigureSchemaError> {
        classification.validate()?;
        self.select_for_type(classification.visual_type.clone())
    }

    /// Provenance helper: schema_id + sha256 of JSON schema body.
    pub fn selection_lineage(
        &self,
        visual_type: VisualAnalysisType,
    ) -> Result<(String, String), FigureSchemaError> {
        let selection = self.select_for_type(visual_type)?;
        let sha = selection.schema_sha256_hex()?;
        Ok((selection.schema_id, sha))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::FigureClassifyMethod;

    fn sha64(seed: u8) -> String {
        format!("{:064x}", seed as u128)
    }

    #[test]
    fn service_selects_chart_schema_for_plot_classification() {
        let svc = FigureSchemaService::bundled().expect("bundled");
        let class = FigureTypeClassification {
            artifact_id: "Figure2".into(),
            visual_type: VisualAnalysisType::Plot,
            confidence: 0.7,
            method: FigureClassifyMethod::Heuristic,
            matched_keywords: vec!["plot".into()],
            subpanel_count: 1,
            panel_labels: vec![],
            labels_schema_version: "figure-type-labels.v1".into(),
            context_sha256: sha64(1),
            image_sha256: sha64(2),
            authoritative: false,
            import_eligible: false,
        };
        let sel = svc.select_for_classification(&class).expect("select");
        assert_eq!(sel.schema_id, "chart_extraction");
        assert!(sel.json_schema.get("properties").is_some());
        let (id, sha) = svc
            .selection_lineage(VisualAnalysisType::Plot)
            .expect("lineage");
        assert_eq!(id, "chart_extraction");
        assert_eq!(sha.len(), 64);
    }

    #[test]
    fn service_selects_diagram_schema_for_flowchart() {
        let svc = FigureSchemaService::bundled().expect("bundled");
        let sel = svc
            .select_for_type(VisualAnalysisType::Flowchart)
            .expect("select");
        assert_eq!(sel.schema_id, "diagram_analysis");
        assert!(sel.required_top_level.iter().any(|f| f == "citations"));
    }
}
