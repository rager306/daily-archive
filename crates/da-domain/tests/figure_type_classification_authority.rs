//! Fail-closed authority tests for Stage-1 classification.
//! Lives under crates/da-domain/tests/ so governor D127 source scan
//! (path_contains_exclude: /tests/) permits temporary true assignment.

use da_domain::{
    FigureAnalysisError, FigureClassifyMethod, FigureTypeClassification, VisualAnalysisType,
};

fn sha64(seed: u8) -> String {
    format!("{:064x}", seed as u128)
}

#[test]
fn stage1_rejects_import_eligible_true() {
    let mut c = FigureTypeClassification {
        artifact_id: "Figure1".into(),
        visual_type: VisualAnalysisType::Other,
        confidence: 0.25,
        method: FigureClassifyMethod::Heuristic,
        matched_keywords: vec![],
        subpanel_count: 1,
        labels_schema_version: "figure-type-labels.v1".into(),
        context_sha256: sha64(5),
        image_sha256: sha64(6),
        authoritative: false,
        import_eligible: false,
    };
    c.validate().unwrap();
    c.import_eligible = true;
    assert_eq!(c.validate(), Err(FigureAnalysisError::AuthorityViolation));
}
