//! M400 S02 T03 closure proof: the revision-grounded TableCandidate
//! materializer writes non-authoritative TableCandidate + TableCell +
//! TableConflict nodes over a real revision plane and replays idempotently.
//!
//! The proof exercises the real boundaries, not hand-built fixtures:
//! RevisionMaterializationUseCase materializes the PaperRevision + Sections
//! (the grounding targets), the ports→domain conversion
//! (`structural_table_from_extracted`) projects a deterministic TEI table,
//! `compare_table_fidelity` produces typed fidelity evidence, and
//! `TableCandidate::assemble` builds the candidate that
//! `TableCandidateMaterializer` writes into the graph. Candidate nodes expose
//! revision, section, image, TEI lineage, coverage, per-cell row/column/span/
//! digest evidence, typed conflicts, and authority locks — and an identical
//! replay applies zero writes.

mod common;

use std::sync::Arc;

use common::mock_graph_store::MockGraphStore;
use da_application::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationUseCase,
};
use da_application::table_materialize::{
    DIAGNOSTIC_APPLIED, DIAGNOSTIC_UNCHANGED_REPLAY, TABLE_CANDIDATE_LABEL, TABLE_CELL_LABEL,
    TABLE_CONFLICT_LABEL, TableCandidateMaterializer, TableMaterializationError,
    structural_table_from_extracted,
};
use da_domain::figure_analysis::FigureTablePayload;
use da_domain::relation::{conflict, structure};
use da_domain::table_candidate::{
    TABLE_CANDIDATE_SCHEMA_VERSION, TableCandidate, TableCandidateLineage,
};
use da_domain::table_fidelity::compare_table_fidelity;
use da_domain::vid::revision_section_vid;
use da_ports::clock::FixedClock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{CharSpan, ExtractedTable, ParsedArticle, Section, TableCell};

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

fn ports_cell(row: u32, column: u32, text: &str) -> TableCell {
    TableCell {
        row,
        column,
        colspan: 1,
        rowspan: 1,
        text: text.into(),
        coordinates: Some(format!(
            "{row},{column},{},{};3",
            column * 100,
            row * 100 + 50
        )),
        span: Some(CharSpan {
            start: row * 100 + column * 10,
            end: row * 100 + column * 10 + text.len() as u32,
        }),
        digest: format!("{row:032x}{column:032x}"),
    }
}

fn extracted_table(cells: Vec<TableCell>) -> ExtractedTable {
    ExtractedTable {
        table_id: "tab_0".into(),
        section_path_id: Some("2".into()),
        section_ordinal: Some(2),
        caption: "Table 3 :".into(),
        coordinates: Some("10,20,500,300;3".into()),
        cells,
        source_digest: sha(7),
        missingness: None,
    }
}

fn visual(headers: &[&str], rows: &[&[&str]]) -> FigureTablePayload {
    FigureTablePayload {
        headers: headers.iter().map(|s| s.to_string()).collect(),
        rows: rows
            .iter()
            .map(|row| row.iter().map(|s| s.to_string()).collect())
            .collect(),
    }
}

fn lineage(revision_vid: &str) -> TableCandidateLineage {
    TableCandidateLineage {
        revision_vid: revision_vid.into(),
        artifact_id: "Table3".into(),
        detector_name: "pdffigures2".into(),
        detector_fingerprint: "pdffigures2-0.0.12".into(),
        image_sha256: sha(7),
        asset_path: "figures/Table3.png".into(),
        page_index: 3,
        section_vid: revision_section_vid(revision_vid, 2).unwrap(),
        section_title: "Section 2".into(),
        section_ordinal: 2,
        caption: "Table 3: GEPA results.".into(),
        source_pdf_sha256: sha(1),
    }
}

/// The canonical positive fixture grid cells (2x3, no findings).
fn perfect_cells() -> Vec<TableCell> {
    vec![
        ports_cell(0, 0, "Method"),
        ports_cell(0, 1, "F1"),
        ports_cell(1, 0, "GEPA"),
        ports_cell(1, 1, "0.87"),
        ports_cell(2, 0, "Baseline"),
        ports_cell(2, 1, "0.72"),
    ]
}

/// The canonical positive fixture visual payload (perfect agreement).
fn perfect_visual() -> FigureTablePayload {
    visual(
        &["Method", "F1"],
        &[&["GEPA", "0.87"], &["Baseline", "0.72"]],
    )
}

/// Full pipeline: ports table → domain projection → fidelity report →
/// assembled candidate. `visual_payload` and `tei_cells` allow building the
/// positive (perfect grid) and negative (conflicting) fixtures.
fn assemble_candidate(
    lineage: &TableCandidateLineage,
    tei_cells: Vec<TableCell>,
    visual_payload: Option<FigureTablePayload>,
) -> TableCandidate {
    let extracted = extracted_table(tei_cells);
    let structural = structural_table_from_extracted(&extracted);
    structural.validate().expect("structural projection valid");
    let report = compare_table_fidelity(visual_payload.as_ref(), Some(&structural))
        .expect("fidelity comparison");
    TableCandidate::assemble(
        lineage,
        visual_payload.as_ref(),
        &structural,
        &report,
        extracted.coordinates.clone(),
    )
    .expect("assemble candidate")
}

/// The canonical positive fixture: a perfect 2x3 grid with zero findings.
fn perfect_candidate(revision_vid: &str) -> TableCandidate {
    assemble_candidate(
        &lineage(revision_vid),
        perfect_cells(),
        Some(perfect_visual()),
    )
}

fn materializer(store: MockGraphStore) -> TableCandidateMaterializer {
    TableCandidateMaterializer::new(Arc::new(store))
}

#[tokio::test]
async fn revision_grounded_table_candidates_expose_full_lineage_and_locks() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    let candidate = perfect_candidate(&revision);
    let result = materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");

    assert!(result.applied && !result.unchanged);
    assert_eq!(result.nodes_created, 7, "candidate + 6 cells");
    assert_eq!(result.edges_created, 8, "lineage + cell edges");
    assert_eq!(result.cells_created, 6);
    assert_eq!(result.conflicts_created, 0);
    assert_eq!(result.materialization_diagnostics, DIAGNOSTIC_APPLIED);
    assert!(
        !result.import_eligible,
        "D127: candidate is never import eligible"
    );

    // Node exposes revision, section, image, TEI lineage, coverage, schema,
    // and authority-lock evidence.
    let node = store
        .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .expect("candidate node");
    assert_eq!(
        store.get_node_property_string(node, "revision_vid").await,
        Some(revision.clone())
    );
    assert_eq!(
        store.get_node_property_string(node, "section_vid").await,
        Some(candidate.section_vid.clone())
    );
    assert_eq!(
        store.get_node_property_int(node, "section_ordinal").await,
        Some(2)
    );
    assert_eq!(
        store.get_node_property_string(node, "image_sha256").await,
        Some(sha(7))
    );
    assert_eq!(
        store.get_node_property_string(node, "tei_table_id").await,
        Some("tab_0".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "tei_source_digest")
            .await,
        Some(sha(7))
    );
    assert_eq!(
        store
            .get_node_property_string(node, "tei_coordinates")
            .await,
        Some("10,20,500,300;3".into())
    );
    assert_eq!(
        store
            .get_node_property_int(node, "coverage_visual_cells")
            .await,
        Some(6)
    );
    assert_eq!(
        store.get_node_property_int(node, "coverage_matched").await,
        Some(6)
    );
    assert_eq!(
        store
            .get_node_property_int(node, "coverage_unmatched_visual")
            .await,
        Some(0)
    );
    assert_eq!(
        store
            .get_node_property_int(node, "coverage_unmatched_tei")
            .await,
        Some(0)
    );
    assert_eq!(
        store.get_node_property_int(node, "schema_version").await,
        Some(TABLE_CANDIDATE_SCHEMA_VERSION as i64)
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
        "PaperRevision hasPart TableCandidate"
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
        "TableCandidate groundedIn Section"
    );
}

#[tokio::test]
async fn cells_carry_row_column_span_and_digest_evidence() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let candidate = perfect_candidate(&revision);
    materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");

    // Every structural cell is a TableCell node owned by the candidate via
    // hasPart, carrying row/column/span/digest + coordinates + verbatim texts.
    let cell = candidate
        .cells
        .iter()
        .find(|c| c.row == 1 && c.column == 1)
        .expect("cell (1,1)");
    let cell_node = store
        .find_node_by_string_property(TABLE_CELL_LABEL, "vid", &cell.vid)
        .await
        .expect("cell node");
    assert_eq!(store.get_node_property_int(cell_node, "row").await, Some(1));
    assert_eq!(
        store.get_node_property_int(cell_node, "column").await,
        Some(1)
    );
    assert_eq!(
        store.get_node_property_string(cell_node, "text").await,
        Some("0.87".into())
    );
    assert_eq!(
        store
            .get_node_property_string(cell_node, "visual_text")
            .await,
        Some("0.87".into()),
        "visual text preserved verbatim on agreement"
    );
    assert_eq!(
        store.get_node_property_int(cell_node, "span_start").await,
        Some(110)
    );
    assert_eq!(
        store.get_node_property_int(cell_node, "span_end").await,
        Some(114)
    );
    assert_eq!(
        store.get_node_property_string(cell_node, "digest").await,
        Some(format!("{:032x}{:032x}", 1u32, 1u32))
    );
    assert_eq!(
        store
            .get_node_property_string(cell_node, "coordinates")
            .await,
        Some("1,1,100,150;3".into())
    );
    assert_eq!(
        store
            .get_node_property_bool(cell_node, "authoritative")
            .await,
        Some(false)
    );
    assert_eq!(
        store
            .get_node_property_bool(cell_node, "import_eligible")
            .await,
        Some(false)
    );

    let candidate_node = store
        .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .expect("candidate node");
    assert_eq!(
        store
            .get_edges_between(candidate_node, cell_node, structure::HAS_PART)
            .await
            .len(),
        1,
        "TableCandidate hasPart TableCell"
    );
    assert_eq!(store.get_nodes_by_label(TABLE_CELL_LABEL).await.len(), 6);
}

#[tokio::test]
async fn typed_conflicts_materialize_as_conflict_nodes_with_verbatim_texts() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    // Negative fixture: header "F1-score" vs TEI "F1", body "0.87" vs "0.90".
    let candidate = assemble_candidate(
        &lineage(&revision),
        vec![
            ports_cell(0, 0, "Method"),
            ports_cell(0, 1, "F1"),
            ports_cell(1, 0, "GEPA"),
            ports_cell(1, 1, "0.90"),
        ],
        Some(visual(&["Method", "F1-score"], &[&["GEPA", "0.87"]])),
    );
    assert_eq!(candidate.conflict_count(), 2);
    let result = materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");
    assert_eq!(result.conflicts_created, 2);
    assert_eq!(
        result.nodes_created,
        1 + 4 + 2,
        "candidate + 4 cells + 2 conflicts"
    );

    let candidate_node = store
        .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .expect("candidate node");
    let conflicts = store.get_nodes_by_label(TABLE_CONFLICT_LABEL).await;
    assert_eq!(
        conflicts.len(),
        2,
        "one TableConflict node per typed conflict"
    );
    let mut kinds = Vec::new();
    for conflict_node in conflicts {
        kinds.push(
            store
                .get_node_property_string(conflict_node, "kind")
                .await
                .unwrap(),
        );
        assert_eq!(
            store
                .get_edges_between(candidate_node, conflict_node, conflict::CONFLICTS_OVER)
                .await
                .len(),
            1,
            "TableCandidate conflictsOver TableConflict"
        );
        assert_eq!(
            store
                .get_node_property_bool(conflict_node, "authoritative")
                .await,
            Some(false)
        );
        assert_eq!(
            store
                .get_node_property_bool(conflict_node, "import_eligible")
                .await,
            Some(false)
        );
    }
    kinds.sort();
    assert_eq!(kinds, vec!["header_mismatch", "numeric_mismatch"]);

    let header_evidence = candidate
        .conflicts
        .iter()
        .find(|c| c.kind == "header_mismatch")
        .expect("header conflict evidence");
    let header = store
        .find_node_by_string_property(TABLE_CONFLICT_LABEL, "vid", &header_evidence.vid)
        .await
        .expect("header conflict node");
    assert_eq!(
        store.get_node_property_string(header, "visual_text").await,
        Some("F1-score".into()),
        "visual text preserved verbatim"
    );
    assert_eq!(
        store.get_node_property_string(header, "tei_text").await,
        Some("F1".into()),
        "TEI text preserved verbatim — no overwrite"
    );
    assert_eq!(store.get_node_property_int(header, "row").await, Some(0));
    assert_eq!(store.get_node_property_int(header, "column").await, Some(1));

    // The disagreeing cell carries the typed fidelity kind, TEI text verbatim,
    // and the visual text alongside.
    let conflicting_cell = candidate
        .cells
        .iter()
        .find(|c| c.row == 1 && c.column == 1)
        .expect("cell (1,1)");
    assert_eq!(
        conflicting_cell.fidelity_kind.as_deref(),
        Some("numeric_mismatch")
    );
    let cell_node = store
        .find_node_by_string_property(TABLE_CELL_LABEL, "vid", &conflicting_cell.vid)
        .await
        .expect("cell node");
    assert_eq!(
        store.get_node_property_string(cell_node, "text").await,
        Some("0.90".into()),
        "TEI text never overwritten by visual text"
    );
    assert_eq!(
        store
            .get_node_property_string(cell_node, "visual_text")
            .await,
        Some("0.87".into())
    );
    assert_eq!(
        store
            .get_node_property_string(cell_node, "fidelity_kind")
            .await,
        Some("numeric_mismatch".into())
    );
}

#[tokio::test]
async fn no_visual_payload_missingness_keeps_tei_cells_materialized() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    let candidate = assemble_candidate(
        &lineage(&revision),
        vec![ports_cell(0, 0, "Method"), ports_cell(0, 1, "F1")],
        None,
    );
    assert_eq!(
        candidate.missingness_kind.as_deref(),
        Some("no_visual_payload")
    );
    let result = materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");
    assert_eq!(result.cells_created, 2);
    assert_eq!(result.conflicts_created, 0);

    let node = store
        .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .expect("candidate node");
    assert_eq!(
        store
            .get_node_property_string(node, "missingness_kind")
            .await,
        Some("no_visual_payload".into())
    );
    // TEI cells stay materialized without visual evidence.
    let cells = store.get_nodes_by_label(TABLE_CELL_LABEL).await;
    assert_eq!(cells.len(), 2);
    for cell_node in cells {
        assert_eq!(
            store
                .get_node_property_string(cell_node, "visual_text")
                .await,
            None,
            "no visual text without a visual payload"
        );
        assert_eq!(
            store
                .get_node_property_string(cell_node, "fidelity_kind")
                .await,
            None
        );
    }
}

#[tokio::test]
async fn identical_replay_applies_zero_writes_with_unchanged_evidence() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let materializer = materializer(store.clone());
    let candidate = perfect_candidate(&revision);

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
    assert_eq!(replay.cells_created, 0);
    assert_eq!(replay.conflicts_created, 0);
    assert!(replay.nodes_reused >= 7, "candidate + 6 cells reused");
    assert!(replay.edges_reused >= 8, "2 lineage + 6 cell edges reused");
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

    // Revision not materialized in the graph at all.
    let ghost = perfect_candidate(&sha(99));
    assert!(matches!(
        materializer.materialize(&ghost).await,
        Err(TableMaterializationError::MissingRevision(_))
    ));

    // Revision exists, but the revision-scoped section does not.
    let revision = seed_revision_plane(&store, 1, 2).await;
    let missing_section = perfect_candidate(&revision);
    assert!(matches!(
        materializer.materialize(&missing_section).await,
        Err(TableMaterializationError::MissingSection(_))
    ));
    // No candidate node was written.
    assert!(
        store
            .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &ghost.vid)
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
    // the assembly path with the foreign section lineage so cells/conflicts
    // carry one consistent identity (tampering a vid post-assembly would
    // cascade into cell identity mismatches by design).
    let mut foreign_lineage = lineage(&v1);
    foreign_lineage.section_vid = revision_section_vid(&v2, 2).unwrap();
    let cross = assemble_candidate(&foreign_lineage, perfect_cells(), Some(perfect_visual()));
    assert_ne!(
        cross.revision_vid, cross.section_vid,
        "foreign section lineage"
    );
    assert!(matches!(
        materializer.materialize(&cross).await,
        Err(TableMaterializationError::SectionRevisionMismatch { .. })
    ));
}

#[tokio::test]
async fn divergent_replay_is_rejected_as_conflict() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let revision = seed_revision_plane(&store, 1, 3).await;
    let candidate = perfect_candidate(&revision);
    materializer.materialize(&candidate).await.unwrap();

    // Same VID but divergent caption (tampered lineage payload).
    let mut tampered = candidate.clone();
    tampered.caption = "Table 3: rewritten caption.".into();
    tampered.candidate_metadata_sha256 = sha(99);
    assert!(matches!(
        materializer.materialize(&tampered).await,
        Err(TableMaterializationError::Conflict(_))
    ));
    // The stored node must be untouched.
    let node = store
        .find_node_by_string_property(TABLE_CANDIDATE_LABEL, "vid", &candidate.vid)
        .await
        .unwrap();
    assert_eq!(
        store.get_node_property_string(node, "caption").await,
        Some(candidate.caption)
    );
}

#[tokio::test]
async fn partially_written_candidate_is_rejected_on_replay_as_conflict() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let revision = seed_revision_plane(&store, 1, 3).await;
    let candidate = perfect_candidate(&revision);

    // Simulate a partial write: candidate node without its cells.
    let candidate_node = store.create_node(TABLE_CANDIDATE_LABEL).await.unwrap();
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
        Err(TableMaterializationError::Conflict(_))
    ));
}

#[tokio::test]
async fn two_revisions_produce_disjoint_queryable_candidates() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let v1 = seed_revision_plane(&store, 1, 3).await;
    let v2 = seed_revision_plane(&store, 2, 3).await;

    let a = perfect_candidate(&v1);
    let b = perfect_candidate(&v2);
    assert_ne!(a.vid, b.vid, "revision-bound identity is disjoint");

    materializer.materialize(&a).await.unwrap();
    materializer.materialize(&b).await.unwrap();
    assert_eq!(
        store.get_nodes_by_label(TABLE_CANDIDATE_LABEL).await.len(),
        2
    );

    let mut revision_vids = std::collections::HashSet::new();
    for node in store.get_nodes_by_label(TABLE_CANDIDATE_LABEL).await {
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

    let mut unlocked = perfect_candidate(&revision);
    unlocked.authoritative = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(TableMaterializationError::Invalid(_))
    ));
    let mut unlocked = perfect_candidate(&revision);
    unlocked.import_eligible = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(TableMaterializationError::Invalid(_))
    ));
    // No candidate node was written for the rejected identity.
    assert!(
        store
            .find_node_by_string_property(
                TABLE_CANDIDATE_LABEL,
                "vid",
                &perfect_candidate(&revision).vid
            )
            .await
            .is_none()
    );
}

#[tokio::test]
async fn materialization_result_roundtrips() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let result = materializer(store)
        .materialize(&perfect_candidate(&revision))
        .await
        .unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let decoded: da_application::table_materialize::TableMaterializationResult =
        serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, result);
    assert!(!decoded.import_eligible);
}
