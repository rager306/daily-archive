use std::collections::BTreeMap;

use da_domain::{
    assurance::{BoundaryMode, ContentDigest},
    context_conservation::StageContextSnapshot,
};

#[test]
fn context_projection_cannot_authorize_import() {
    let snapshot = StageContextSnapshot {
        stage_id: "parse".into(),
        mode: BoundaryMode::NonAuthoritativeDiagnostic,
        artifact_sha256: ContentDigest::of_text("artifact"),
        source_bytes: 1,
        text_chars: 1,
        token_count: None,
        tokenizer_lineage: None,
        sections: 1,
        citations: 0,
        figures: 0,
        tables: 0,
        grounded_spans: 0,
        grounded_section_vids: 0,
        entities: 0,
        evidence_items: 0,
        summaries: 0,
        manifestations: 0,
        duplicate_section_titles: 0,
        navigation_noise_sections: 0,
        missingness: BTreeMap::new(),
        no_claim: None,
        import_eligible: true,
    };
    assert!(
        snapshot
            .validate()
            .unwrap_err()
            .contains("cannot authorize import")
    );
}
