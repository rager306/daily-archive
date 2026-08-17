//! M400 S03 T03 closure proof: the revision-grounded EquationCandidate
//! materializer writes non-authoritative EquationCandidate nodes over a real
//! revision plane and replays idempotently.
//!
//! The proof exercises the real boundaries, not hand-built fixtures:
//! RevisionMaterializationUseCase materializes the PaperRevision + Sections
//! (the grounding targets), the ports→domain conversion
//! (`equation_candidate_input_from_extracted`) projects a deterministic TEI
//! formula, and `EquationCandidate::new` builds the candidate that
//! `EquationCandidateMaterializer` writes into the graph. Candidate nodes
//! expose formula identity, revision/section lineage, representation kind and
//! digest, source span, extraction missingness (typed conversion errors), and
//! authority locks — and an identical replay applies zero writes.

mod common;

use std::sync::Arc;

use common::mock_graph_store::MockGraphStore;
use da_application::equation_materialize::{
    DIAGNOSTIC_APPLIED, DIAGNOSTIC_UNCHANGED_REPLAY, EQUATION_CANDIDATE_LABEL,
    EquationCandidateMaterializer, EquationMaterializationError,
    equation_candidate_input_from_extracted,
};
use da_application::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationUseCase,
};
use da_domain::equation_candidate::EquationCandidate;
use da_domain::relation::structure;
use da_domain::vid::revision_section_vid;
use da_ports::clock::FixedClock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{CharSpan, ExtractedFormula, FormulaRepresentation, ParsedArticle, Section};

const PAPER_ID: &str = "2507.19457";

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

fn revision_request(revision: u32, section_count: usize) -> RevisionMaterializationRequest {
    RevisionMaterializationRequest {
        source_code: "arxiv".into(),
        canonical_id: PAPER_ID.into(),
        revision,
        parsed: ParsedArticle {
            paper_id: PAPER_ID.into(),
            title: "GEPA".into(),
            abstract_text: "Reflective prompt mutation.".into(),
            body_text: "body".into(),
            sections: (0..section_count)
                .map(|ordinal| Section {
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
            formulas: Vec::new(),
            formula_missingness: None,
            pdf_hash: sha(u64::from(revision)),
        },
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: sha(u64::from(revision)),
            bytes: 1000 + u64::from(revision),
            uri: format!("https://arxiv.org/pdf/{PAPER_ID}v{revision}"),
            path: format!("{PAPER_ID}v{revision}.pdf"),
            capture_status: "captured".into(),
        }],
    }
}

/// Materialize a revision plane (PaperRevision + Sections) exactly as the
/// production revision use case does.
async fn seed_revision_plane(
    store: &MockGraphStore,
    revision: u32,
    section_count: usize,
) -> String {
    let uc = RevisionMaterializationUseCase::new(
        Arc::new(store.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );
    let out = uc
        .materialize(&revision_request(revision, section_count))
        .await
        .expect("revision materialization");
    out.revision_vid
}

/// The canonical deterministic TEI formula fixture: grounded in section
/// ordinal 2 ("Section 2", path "2") with a LaTeX representation and a
/// source span inside that section's text window.
fn ports_formula() -> ExtractedFormula {
    ExtractedFormula {
        formula_id: "formula_0".into(),
        section_path_id: Some("2".into()),
        section_ordinal: Some(2),
        label: Some("(1)".into()),
        coordinates: Some("10,20,30,40;5".into()),
        representation: FormulaRepresentation::TeXMath(
            "L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]".into(),
        ),
        span: Some(CharSpan {
            start: 120,
            end: 158,
        }),
        source_digest: sha(7),
        missingness: None,
    }
}

/// The revision's section list exactly as `revision_request` materializes it
/// (titles, path ids, section ids) — the conversion resolves section lineage
/// against this list by ordinal.
fn sections(section_count: usize) -> Vec<Section> {
    (0..section_count)
        .map(|ordinal| Section {
            title: format!("Section {ordinal}"),
            text: format!("text {ordinal}"),
            level: 1,
            path_id: Some(format!("{ordinal}")),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        })
        .collect()
}

/// Full pipeline: ports formula → domain input → validated candidate.
/// `source_pdf_sha256` defaults to the revision-1 PDF digest so the lineage
/// matches the seeded revision plane.
fn equation_candidate(revision_vid: &str) -> EquationCandidate {
    let input = equation_candidate_input_from_extracted(
        &ports_formula(),
        revision_vid,
        &sha(1),
        &sections(3),
    )
    .expect("ports to domain conversion");
    EquationCandidate::new(input).expect("assemble candidate")
}

fn materializer(store: MockGraphStore) -> EquationCandidateMaterializer {
    EquationCandidateMaterializer::new(Arc::new(store))
}

#[tokio::test]
async fn revision_grounded_equation_candidates_expose_full_lineage_and_locks() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    let candidate = equation_candidate(&revision);
    let result = materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");

    assert!(result.applied && !result.unchanged);
    assert_eq!(result.nodes_created, 1, "one candidate node");
    assert_eq!(result.edges_created, 2, "revision + section lineage edges");
    assert_eq!(result.materialization_diagnostics, DIAGNOSTIC_APPLIED);
    assert!(
        !result.import_eligible,
        "D127: candidate is never import eligible"
    );

    // Node exposes formula identity, revision/section lineage, representation
    // kind + verbatim text + digest, source span, schema, and authority locks.
    let node = store
        .find_node_by_string_property(EQUATION_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .expect("candidate node");
    assert_eq!(
        store.get_node_property_string(node, "revision_vid").await,
        Some(revision.clone())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "source_pdf_sha256")
            .await,
        Some(sha(1))
    );
    assert_eq!(
        store.get_node_property_string(node, "formula_id").await,
        Some("formula_0".into())
    );
    assert_eq!(
        store.get_node_property_string(node, "section_vid").await,
        Some(candidate.section_vid.clone())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "section_path_id")
            .await,
        Some("2".into())
    );
    assert_eq!(
        store.get_node_property_int(node, "section_ordinal").await,
        Some(2)
    );
    assert_eq!(
        store.get_node_property_string(node, "section_title").await,
        Some("Section 2".into())
    );
    assert_eq!(
        store.get_node_property_string(node, "label").await,
        Some("(1)".into())
    );
    assert_eq!(
        store.get_node_property_string(node, "coordinates").await,
        Some("10,20,30,40;5".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "representation_kind")
            .await,
        Some("tex_math".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "representation_text")
            .await,
        Some("L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "representation_sha256")
            .await,
        Some(candidate.representation_sha256.clone())
    );
    assert_eq!(
        store.get_node_property_int(node, "span_start").await,
        Some(120)
    );
    assert_eq!(
        store.get_node_property_int(node, "span_end").await,
        Some(158)
    );
    assert_eq!(
        store.get_node_property_string(node, "source_digest").await,
        Some(sha(7))
    );
    assert_eq!(
        store
            .get_node_property_string(node, "candidate_metadata_sha256")
            .await,
        Some(candidate.candidate_metadata_sha256.clone())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "materialization_diagnostics")
            .await,
        Some(DIAGNOSTIC_APPLIED.into())
    );
    assert_eq!(
        store.get_node_property_bool(node, "authoritative").await,
        Some(false)
    );
    assert_eq!(
        store.get_node_property_bool(node, "import_eligible").await,
        Some(false)
    );

    // Lineage edges: revision owns the candidate; candidate is grounded in the
    // revision-scoped Section.
    let revision_node = store
        .find_node_by_string_property("PaperRevision", "vid", &revision)
        .await
        .expect("revision node");
    assert_eq!(
        store
            .get_edges_between(revision_node, node, structure::HAS_PART)
            .await
            .len(),
        1,
        "PaperRevision hasPart EquationCandidate"
    );
    let section_node = store
        .find_node_by_string_property("Section", "vid", &candidate.section_vid)
        .await
        .expect("section node");
    assert_eq!(
        store
            .get_edges_between(node, section_node, structure::GROUNDED_IN)
            .await
            .len(),
        1,
        "EquationCandidate groundedIn Section"
    );
}

#[tokio::test]
async fn identical_replay_applies_zero_writes_with_unchanged_evidence() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let materializer = materializer(store.clone());
    let candidate = equation_candidate(&revision);

    materializer.materialize(&candidate).await.unwrap();
    let nodes_before = store.node_count_total();
    let edges_before = store.edge_count_total();

    let replay = materializer.materialize(&candidate).await.unwrap();
    assert!(
        !replay.applied && replay.unchanged,
        "applied-zero unchanged-positive"
    );
    assert_eq!(replay.nodes_created, 0);
    assert_eq!(replay.edges_created, 0);
    assert!(replay.nodes_reused >= 1, "candidate node reused");
    assert!(replay.edges_reused >= 2, "2 lineage edges reused");
    assert_eq!(
        replay.materialization_diagnostics,
        DIAGNOSTIC_UNCHANGED_REPLAY
    );
    assert_eq!(store.node_count_total(), nodes_before, "no duplicate nodes");
    assert_eq!(store.edge_count_total(), edges_before, "no duplicate edges");
}

#[tokio::test]
async fn missing_revision_or_section_identity_fails_before_write() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());

    // Revision not materialized in the graph at all. The conversion still
    // resolves deterministic lineage against a standalone section list.
    let ghost = equation_candidate(&sha(99));
    assert!(matches!(
        materializer.materialize(&ghost).await,
        Err(EquationMaterializationError::MissingRevision(_))
    ));

    // Revision exists, but the revision-scoped section does not: seed only 2
    // sections (ordinals 0..1) and bind the candidate to section ordinal 2
    // through the domain assembly path (the domain does not know the section
    // list, so the consistent candidate validates; the graph lookup fails).
    let revision = seed_revision_plane(&store, 1, 2).await;
    // Convert a formula grounded in the materialized section 1, then rebind
    // the section lineage to ordinal 2 (not materialized in this plane).
    let mut formula = ports_formula();
    formula.section_ordinal = Some(1);
    formula.section_path_id = Some("1".into());
    let mut input =
        equation_candidate_input_from_extracted(&formula, &revision, &sha(1), &sections(2))
            .expect("conversion with 2 sections resolves ordinal 1");
    input.section_ordinal = 2;
    input.section_path_id = "2".into();
    input.section_vid = revision_section_vid(&revision, 2).unwrap();
    let missing_section = EquationCandidate::new(input).expect("assemble candidate");
    assert!(matches!(
        materializer.materialize(&missing_section).await,
        Err(EquationMaterializationError::MissingSection(_))
    ));
    // No candidate node was written.
    assert!(
        store
            .find_node_by_string_property(EQUATION_CANDIDATE_LABEL, "vid", &ghost.vid)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn section_from_another_revision_is_rejected_as_broken_lineage() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let v1 = seed_revision_plane(&store, 1, 3).await;
    let v2 = seed_revision_plane(&store, 2, 3).await;

    // Candidate bound to v1 but grounded in v2's section 2: rebuild through
    // the domain assembly path with the foreign section lineage so the
    // deterministic identity stays consistent (tampering a vid post-assembly
    // would be rejected by identity validation by design).
    let mut input =
        equation_candidate_input_from_extracted(&ports_formula(), &v1, &sha(1), &sections(3))
            .expect("conversion");
    input.section_vid = revision_section_vid(&v2, 2).unwrap();
    let cross = EquationCandidate::new(input).expect("assemble candidate");
    assert_ne!(
        cross.revision_vid, cross.section_vid,
        "foreign section lineage"
    );
    assert!(matches!(
        materializer.materialize(&cross).await,
        Err(EquationMaterializationError::SectionRevisionMismatch { .. })
    ));
}

#[tokio::test]
async fn divergent_replay_is_rejected_as_conflict() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let revision = seed_revision_plane(&store, 1, 3).await;
    let candidate = equation_candidate(&revision);
    materializer.materialize(&candidate).await.unwrap();

    // Same VID but divergent representation (tampered lineage payload).
    let mut tampered = candidate.clone();
    tampered.representation_text = "L(\\theta)=\\mathbb{E}[\\log q_\\theta(x)]".into();
    tampered.candidate_metadata_sha256 = sha(99);
    assert!(matches!(
        materializer.materialize(&tampered).await,
        Err(EquationMaterializationError::Conflict(_))
    ));
    // The stored node must be untouched.
    let node = store
        .find_node_by_string_property(EQUATION_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_node_property_string(node, "representation_text")
            .await,
        Some(candidate.representation_text)
    );
}

#[tokio::test]
async fn partially_written_candidate_is_rejected_on_replay_as_conflict() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let revision = seed_revision_plane(&store, 1, 3).await;
    let candidate = equation_candidate(&revision);

    // Simulate a partial write: candidate node with only its identity
    // properties, missing representation/span evidence and lineage edges.
    let candidate_node = store.create_node(EQUATION_CANDIDATE_LABEL).await.unwrap();
    store
        .set_node_property_string(candidate_node, "vid", candidate.vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(candidate_node, "revision_vid", revision.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(candidate_node, "section_vid", candidate.section_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_bool(candidate_node, "authoritative", false)
        .await
        .unwrap();
    store
        .set_node_property_bool(candidate_node, "import_eligible", false)
        .await
        .unwrap();

    assert!(matches!(
        materializer.materialize(&candidate).await,
        Err(EquationMaterializationError::Conflict(_))
    ));
}

#[tokio::test]
async fn two_revisions_produce_disjoint_queryable_candidates() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let v1 = seed_revision_plane(&store, 1, 3).await;
    let v2 = seed_revision_plane(&store, 2, 3).await;

    let a = equation_candidate(&v1);
    let b = equation_candidate(&v2);
    assert_ne!(a.vid, b.vid, "revision-bound identity is disjoint");

    materializer.materialize(&a).await.unwrap();
    materializer.materialize(&b).await.unwrap();
    assert_eq!(
        store
            .get_nodes_by_label(EQUATION_CANDIDATE_LABEL)
            .await
            .len(),
        2
    );

    let mut revision_vids = std::collections::HashSet::new();
    for node in store.get_nodes_by_label(EQUATION_CANDIDATE_LABEL).await {
        revision_vids.insert(
            store
                .get_node_property_string(node, "revision_vid")
                .await
                .unwrap(),
        );
    }
    let expected: std::collections::HashSet<String> =
        [v1.clone(), v2.clone()].into_iter().collect();
    assert_eq!(revision_vids, expected);
}

#[tokio::test]
async fn authority_unlock_is_rejected_and_never_written() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let revision = seed_revision_plane(&store, 1, 3).await;

    let mut unlocked = equation_candidate(&revision);
    unlocked.authoritative = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(EquationMaterializationError::Invalid(_))
    ));
    let mut unlocked = equation_candidate(&revision);
    unlocked.import_eligible = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(EquationMaterializationError::Invalid(_))
    ));
    // No candidate node was written for the rejected identity.
    assert!(
        store
            .find_node_by_string_property(
                EQUATION_CANDIDATE_LABEL,
                "vid",
                &equation_candidate(&revision).vid
            )
            .await
            .is_none()
    );
}

#[tokio::test]
async fn ungrounded_formula_is_typed_missingness_and_never_materializes() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    // Formula outside every section window: no ordinal → no candidate.
    let mut formula = ports_formula();
    formula.section_ordinal = None;
    let error = equation_candidate_input_from_extracted(&formula, &revision, &sha(1), &sections(3))
        .expect_err("ungrounded formula must be typed missingness");
    assert!(matches!(
        error,
        EquationMaterializationError::UngroundedFormula(_)
    ));

    // Formula whose content is unusable: typed malformed missingness.
    let mut formula = ports_formula();
    formula.missingness = Some(da_ports::parser::FormulaMissingReason::Malformed(
        "empty formula with no representation".into(),
    ));
    let error = equation_candidate_input_from_extracted(&formula, &revision, &sha(1), &sections(3))
        .expect_err("malformed formula must be typed missingness");
    assert!(matches!(
        error,
        EquationMaterializationError::MalformedFormula(_, _)
    ));

    // Formula referencing a section ordinal outside the revision's section
    // list: typed out-of-range error.
    let mut formula = ports_formula();
    formula.section_ordinal = Some(9);
    let error = equation_candidate_input_from_extracted(&formula, &revision, &sha(1), &sections(3))
        .expect_err("out-of-range ordinal must be typed missingness");
    assert!(matches!(
        error,
        EquationMaterializationError::SectionOrdinalOutOfRange { .. }
    ));

    // None of the rejected formulas wrote a candidate node.
    assert!(
        store
            .get_nodes_by_label(EQUATION_CANDIDATE_LABEL)
            .await
            .is_empty(),
        "no candidate nodes for rejected formulas"
    );
}

#[tokio::test]
async fn materialization_result_roundtrips() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let result = materializer(store)
        .materialize(&equation_candidate(&revision))
        .await
        .unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let decoded: da_application::equation_materialize::EquationMaterializationResult =
        serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, result);
    assert!(!decoded.import_eligible);
}
