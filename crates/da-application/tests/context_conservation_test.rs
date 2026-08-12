use std::collections::{BTreeMap, BTreeSet};

use da_application::context_conservation::{
    observe_extracted_entities, observe_parsed_article, observe_summary_readiness,
};
use da_domain::{
    entity::EntityType,
    extractive_summary::{ExtractiveSummaryBatch, ExtractiveSummaryUnit},
    revision_readiness::{
        RevisionReadinessAssessment, RevisionReadinessObservation, RevisionReadinessState,
    },
};
use da_ports::{
    extractor::ExtractedEntity,
    parser::{ParsedArticle, Section},
};

fn parsed() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2506.07853".into(),
        title: "Study".into(),
        abstract_text: "A grounded abstract.".into(),
        body_text: "Method alpha. Method beta. References.".into(),
        sections: vec![
            Section {
                title: "Method".into(),
                text: "Method alpha.".into(),
                ..Default::default()
            },
            Section {
                title: "Method".into(),
                text: "Method beta.".into(),
                ..Default::default()
            },
            Section {
                title: "References".into(),
                text: "Citation.".into(),
                ..Default::default()
            },
        ],
        citations: vec![],
        layout_json: None,
        tei_xml: Some("<figure></figure><figure type=\"table\"></figure>".into()),
        pdf_hash: "a".repeat(64),
    }
}

#[test]
fn parse_observer_reports_duplicates_navigation_and_typed_missingness() {
    let noise = BTreeSet::from(["references".into()]);
    let snapshot = observe_parsed_article(&parsed(), 123, &noise).unwrap();
    assert_eq!(snapshot.duplicate_section_titles, 1);
    assert_eq!(snapshot.navigation_noise_sections, 1);
    assert_eq!(snapshot.figures, 2);
    assert_eq!(snapshot.tables, 1);
    assert_eq!(
        snapshot.missingness["citations_missing"],
        da_domain::assurance::MissingnessReason::GenuinelyAbsent
    );
    assert_eq!(
        snapshot.missingness["navigation_identity_missing"],
        da_domain::assurance::MissingnessReason::NotMaterialized
    );
    assert!(!snapshot.import_eligible);
}

#[test]
fn extraction_observer_requires_document_scoped_ordinal_and_valid_byte_span() {
    let entities = vec![
        ExtractedEntity {
            label: "alpha".into(),
            entity_type: EntityType::Method,
            section_title: "Method".into(),
            section_ordinal: Some(0),
            char_start: 7,
            char_end: 12,
            surface: "alpha".into(),
        },
        ExtractedEntity {
            label: "bad".into(),
            entity_type: EntityType::Method,
            section_title: "Method".into(),
            section_ordinal: None,
            char_start: 0,
            char_end: 3,
            surface: "bad".into(),
        },
    ];
    let snapshot = observe_extracted_entities(&parsed(), &entities).unwrap();
    assert_eq!(snapshot.grounded_spans, 1);
    assert_eq!(snapshot.grounded_section_vids, 1);
    assert_eq!(
        snapshot.missingness["entity_section_ordinal_missing"],
        da_domain::assurance::MissingnessReason::NotMaterialized
    );
}

#[test]
fn summary_readiness_observer_preserves_grounding_and_blockers() {
    let unit = ExtractiveSummaryUnit {
        path_id: "1".into(),
        title: "Method".into(),
        text: "Method alpha.".into(),
        granularity: "section".into(),
        source_label: "Section".into(),
        source_id: "vid:section:x:0".into(),
        method: "lead".into(),
        char_count: 13,
        source_text_sha256: "a".repeat(64),
        source_chars: 13,
    };
    let batch = ExtractiveSummaryBatch {
        schema_version: "v1".into(),
        paper_id: "x".into(),
        abstract_summary: None,
        section_summaries: vec![unit],
        skipped_empty_sections: 0,
    };
    let observation = RevisionReadinessObservation {
        revision_vid: "vid:revision:x".into(),
        section_count: 3,
        grounded_summary_count: 1,
        summary_type_counts: BTreeMap::new(),
        manifestation_count: 1,
    };
    let assessment = RevisionReadinessAssessment {
        state: RevisionReadinessState::Partial,
        summary_coverage_bps: 3333,
        blocker_codes: vec![],
        diagnostic_codes: vec!["summary_coverage_low".into()],
    };
    let snapshot = observe_summary_readiness(&batch, &observation, &assessment).unwrap();
    assert_eq!(snapshot.summaries, 1);
    assert_eq!(snapshot.grounded_section_vids, 1);
    assert_eq!(
        snapshot.missingness["summary_coverage_low"],
        da_domain::assurance::MissingnessReason::PolicySkipped
    );
}
