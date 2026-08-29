//! M400 S01 T03 closure proof: the revision-grounded FigureCandidate
//! materializer writes non-authoritative candidate nodes over a real
//! revision plane and replays idempotently.
//!
//! The proof exercises the real boundaries, not hand-built fixtures:
//! RevisionMaterializationUseCase materializes the PaperRevision + Sections
//! (the grounding targets), then FigureCandidateMaterializer binds
//! candidates to that revision and section identity. Candidate nodes expose
//! artifact, detector, image, metadata, revision, section, schema, and
//! authority-lock evidence, and an identical replay applies zero writes.

mod common;

use std::sync::Arc;

use common::mock_graph_store::MockGraphStore;
use da_adapters::SamyamaGraphStore;
use da_application::figure_materialize::{
    DIAGNOSTIC_APPLIED, DIAGNOSTIC_UNCHANGED_REPLAY, FigureCandidateMaterializer,
    FigureMaterializationError,
};
use da_application::revision_materialization::{
    RevisionManifestationInput, RevisionMaterializationRequest, RevisionMaterializationUseCase,
};
use da_domain::VisualArtifactKind;
use da_domain::figure_candidate::{FigureCandidate, figure_candidate_vid};
use da_domain::relation::structure;
use da_domain::vid::revision_section_vid;
use da_ports::clock::FixedClock;
use da_ports::graph_store::{DirectGraphStore, GraphStore};
use da_ports::parser::{ParsedArticle, Section};

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
            page_text_missingness: None,
            pdf_hash: sha(u64::from(revision)),
            ..Default::default()
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

/// Build a candidate bound to `revision_vid`, grounded in the section with
/// the given one-based ordinal.
fn candidate(revision_vid: &str, section_ordinal: u32) -> FigureCandidate {
    let section_vid = revision_section_vid(revision_vid, section_ordinal as usize).unwrap();
    let vid = figure_candidate_vid(
        revision_vid,
        "Figure3",
        "pdffigures2-0.0.12",
        &sha(7),
        &section_vid,
    )
    .unwrap();
    FigureCandidate {
        vid,
        revision_vid: revision_vid.into(),
        artifact_id: "Figure3".into(),
        kind: VisualArtifactKind::Figure,
        detector_name: "pdffigures2".into(),
        detector_fingerprint: "pdffigures2-0.0.12".into(),
        detector_metadata_sha256: sha(3),
        source_pdf_sha256: sha(1),
        image_sha256: sha(7),
        asset_path: "figures/Figure3.png".into(),
        page_index: 3,
        section_vid,
        section_title: "Section 2".into(),
        section_ordinal,
        caption: "Figure 3: Overview of the GEPA system.".into(),
        candidate_metadata_sha256: sha(4),
        authoritative: false,
        import_eligible: false,
        schema_version: da_domain::figure_candidate::FIGURE_CANDIDATE_SCHEMA_VERSION,
        materialization_diagnostics: None,
    }
}

fn materializer(store: MockGraphStore) -> FigureCandidateMaterializer {
    FigureCandidateMaterializer::new(Arc::new(store))
}

#[tokio::test]
async fn revision_grounded_candidates_expose_full_lineage_and_locks() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;

    let candidate = candidate(&revision, 2);
    let result = materializer(store.clone())
        .materialize(&candidate)
        .await
        .expect("materialize");

    assert!(result.applied && !result.unchanged);
    assert_eq!(result.nodes_created, 1);
    assert_eq!(result.edges_created, 2);
    assert_eq!(result.materialization_diagnostics, DIAGNOSTIC_APPLIED);
    assert!(
        !result.import_eligible,
        "D127: candidate is never import eligible"
    );

    // Node exposes artifact, detector, image, metadata, revision, section,
    // schema, and authority-lock evidence.
    let node = store
        .find_node_by_string_property("FigureCandidate", "vid", &candidate.vid)
        .await
        .expect("candidate node");
    assert_eq!(
        store.get_node_property_string(node, "revision_vid").await,
        Some(revision.clone())
    );
    assert_eq!(
        store.get_node_property_string(node, "artifact_id").await,
        Some("Figure3".into())
    );
    assert_eq!(
        store.get_node_property_string(node, "detector_name").await,
        Some("pdffigures2".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "detector_fingerprint")
            .await,
        Some("pdffigures2-0.0.12".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "detector_metadata_sha256")
            .await,
        Some(sha(3))
    );
    assert_eq!(
        store
            .get_node_property_string(node, "source_pdf_sha256")
            .await,
        Some(sha(1))
    );
    assert_eq!(
        store.get_node_property_string(node, "image_sha256").await,
        Some(sha(7))
    );
    assert_eq!(
        store.get_node_property_string(node, "asset_path").await,
        Some("figures/Figure3.png".into())
    );
    assert_eq!(
        store.get_node_property_string(node, "kind").await,
        Some("figure".into())
    );
    assert_eq!(
        store.get_node_property_int(node, "page_index").await,
        Some(3)
    );
    assert_eq!(
        store.get_node_property_string(node, "section_vid").await,
        Some(candidate.section_vid.clone())
    );
    assert_eq!(
        store.get_node_property_string(node, "section_title").await,
        Some("Section 2".into())
    );
    assert_eq!(
        store.get_node_property_int(node, "section_ordinal").await,
        Some(2)
    );
    assert_eq!(
        store.get_node_property_string(node, "caption").await,
        Some("Figure 3: Overview of the GEPA system.".into())
    );
    assert_eq!(
        store
            .get_node_property_string(node, "candidate_metadata_sha256")
            .await,
        Some(sha(4))
    );
    assert_eq!(
        store.get_node_property_int(node, "schema_version").await,
        Some(1)
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

    // Exact lineage edges: revision owns the candidate; candidate is grounded
    // in the revision-scoped Section.
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
        "PaperRevision hasPart FigureCandidate"
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
        "FigureCandidate groundedIn Section"
    );
}

#[tokio::test]
async fn identical_replay_applies_zero_writes_with_unchanged_evidence() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let materializer = materializer(store.clone());
    let candidate = candidate(&revision, 2);

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
    assert!(replay.nodes_reused >= 1);
    assert!(replay.edges_reused >= 2);
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
    let ghost = candidate(&sha(99), 2);
    assert!(matches!(
        materializer.materialize(&ghost).await,
        Err(FigureMaterializationError::MissingRevision(_))
    ));

    // Revision exists, but the revision-scoped section does not.
    let revision = seed_revision_plane(&store, 1, 2).await;
    let missing_section = candidate(&revision, 7);
    assert!(matches!(
        materializer.materialize(&missing_section).await,
        Err(FigureMaterializationError::MissingSection(_))
    ));
}

#[tokio::test]
async fn section_from_another_revision_is_rejected_as_broken_lineage() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let v1 = seed_revision_plane(&store, 1, 3).await;
    let v2 = seed_revision_plane(&store, 2, 3).await;

    // Candidate bound to v1 but grounded in v2's section 2.
    let foreign_section_vid = revision_section_vid(&v2, 2).unwrap();
    let mut cross = candidate(&v1, 2);
    cross.section_vid = foreign_section_vid;
    cross.vid = figure_candidate_vid(
        &v1,
        "Figure3",
        "pdffigures2-0.0.12",
        &sha(7),
        &cross.section_vid,
    )
    .unwrap();
    assert!(matches!(
        materializer.materialize(&cross).await,
        Err(FigureMaterializationError::SectionRevisionMismatch { .. })
    ));
}

#[tokio::test]
async fn two_revisions_produce_disjoint_queryable_candidates() {
    let store = MockGraphStore::new();
    let materializer = materializer(store.clone());
    let v1 = seed_revision_plane(&store, 1, 3).await;
    let v2 = seed_revision_plane(&store, 2, 3).await;

    let a = candidate(&v1, 2);
    let b = candidate(&v2, 2);
    assert_ne!(a.vid, b.vid, "revision-bound identity is disjoint");

    materializer.materialize(&a).await.unwrap();
    materializer.materialize(&b).await.unwrap();
    assert_eq!(store.get_nodes_by_label("FigureCandidate").await.len(), 2);

    let mut revision_vids = std::collections::HashSet::new();
    for node in store.get_nodes_by_label("FigureCandidate").await {
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

    let mut unlocked = candidate(&revision, 2);
    unlocked.authoritative = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(FigureMaterializationError::Invalid(_))
    ));
    let mut unlocked = candidate(&revision, 2);
    unlocked.import_eligible = true;
    assert!(matches!(
        materializer.materialize(&unlocked).await,
        Err(FigureMaterializationError::Invalid(_))
    ));
    // No candidate node was written for the rejected identity.
    assert!(
        store
            .find_node_by_string_property("FigureCandidate", "vid", &candidate(&revision, 2).vid)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn materialization_result_roundtrips() {
    let store = MockGraphStore::new();
    let revision = seed_revision_plane(&store, 1, 3).await;
    let result = materializer(store)
        .materialize(&candidate(&revision, 2))
        .await
        .unwrap();
    let json = serde_json::to_string(&result).unwrap();
    let decoded: da_application::figure_materialize::FigureMaterializationResult =
        serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, result);
    assert!(!decoded.import_eligible);
}

// ────────────────────────────────────────────────────────────────
// T04: same-process semantic graph task (figure_candidate.tasks.cypher)
// ────────────────────────────────────────────────────────────────

/// One parsed task from `data/graph_tasks/figure_candidate.tasks.cypher`.
/// Mirrors the canonical `// @query` suite format (da-cli graph_tasks).
struct SuiteTask {
    title: String,
    cypher: String,
    min_rows: usize,
    expect_rows: Option<usize>,
    expect_value: Option<f64>,
    expect_min_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Verdict {
    Pass,
    Empty,
    Error,
}

/// Minimal parser for the `// @query` suite format. This lives in the test
/// because da-application cannot depend on da-cli (onion): it must
/// independently validate the committed suite file.
fn parse_suite(source: &str) -> Vec<SuiteTask> {
    let mut tasks = Vec::new();
    let mut current: Option<SuiteTask> = None;
    let mut body: Vec<String> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// @query") {
            if let Some(mut task) = current.take() {
                task.cypher = body.join("\n").trim().to_string();
                tasks.push(task);
            }
            body.clear();
            let title = rest
                .trim()
                .split('|')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            current = Some(SuiteTask {
                title,
                cypher: String::new(),
                min_rows: 1,
                expect_rows: None,
                expect_value: None,
                expect_min_value: None,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("// @class") {
            let _ = rest; // class directive is preserved for finalize; the test gates on verdicts
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("// @expect_rows") {
            if let Some(task) = current.as_mut() {
                task.expect_rows = rest.trim().parse().ok();
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("// @expect_value") {
            if let Some(task) = current.as_mut() {
                task.expect_value = rest.trim().parse().ok();
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("// @expect_min_value") {
            if let Some(task) = current.as_mut() {
                task.expect_min_value = rest.trim().parse().ok();
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("// @min_rows") {
            if let Some(task) = current.as_mut() {
                task.min_rows = rest.trim().parse().unwrap_or(1);
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if current.is_some() {
            body.push(line.to_string());
        }
    }
    if let Some(mut task) = current {
        task.cypher = body.join("\n").trim().to_string();
        tasks.push(task);
    }
    tasks
}

/// Mirrors `GraphTask::classify` (da-cli): min_rows, count-of-zero, then
/// the exact expect bounds.
fn classify(task: &SuiteTask, rows: usize, first_cell_number: Option<f64>) -> Verdict {
    if rows < task.min_rows {
        return Verdict::Empty;
    }
    if task.expect_value.is_none() {
        // Single numeric cell equal to 0 is not presence.
        if rows == 1 && first_cell_number == Some(0.0) {
            return Verdict::Empty;
        }
    }
    if let Some(expected) = task.expect_rows
        && rows != expected
    {
        return Verdict::Empty;
    }
    if let Some(expected) = task.expect_value {
        match first_cell_number {
            Some(actual) if actual == expected => {}
            _ => return Verdict::Empty,
        }
    }
    if let Some(min_value) = task.expect_min_value {
        match first_cell_number {
            Some(actual) if actual >= min_value => {}
            _ => return Verdict::Empty,
        }
    }
    Verdict::Pass
}

fn cell_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

async fn run_task(
    store: &SamyamaGraphStore,
    task: &SuiteTask,
) -> (Verdict, usize, Option<f64>, Option<String>) {
    match store.query_readonly("daily_archive", &task.cypher).await {
        Ok(result) => {
            let rows = result.records.len();
            let first = result
                .records
                .first()
                .and_then(|record| record.first())
                .and_then(cell_number);
            (classify(task, rows, first), rows, first, None)
        }
        Err(error) => (Verdict::Error, 0, None, Some(error.to_string())),
    }
}

/// Seed a revision plane on the real in-memory Samyama engine exactly as the
/// production revision use case does (same as `seed_revision_plane` but for
/// `SamyamaGraphStore`).
async fn seed_revision_plane_samyama(
    store: &Arc<SamyamaGraphStore>,
    revision: u32,
    section_count: usize,
) -> String {
    let uc = RevisionMaterializationUseCase::new(
        store.clone() as Arc<dyn DirectGraphStore>,
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );
    let out = uc
        .materialize(&revision_request(revision, section_count))
        .await
        .expect("revision materialization");
    out.revision_vid
}

/// T04: the same-process semantic graph task returns grounded Figure
/// candidates with exact section lineage, fails when revision/section
/// identity is missing, and keeps D127 false in a durable task report.
#[tokio::test]
async fn figure_candidate_graph_task() {
    // 1. Load the committed suite file (repo-root relative).
    let suite_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/graph_tasks/figure_candidate.tasks.cypher");
    let source = std::fs::read_to_string(&suite_path).expect("read figure_candidate.tasks.cypher");
    let tasks = parse_suite(&source);
    assert!(
        tasks.len() >= 7,
        "expected >= 7 semantic tasks, parsed {}",
        tasks.len()
    );
    for task in &tasks {
        assert!(
            !task.cypher.trim().is_empty(),
            "task {:?} has empty cypher body",
            task.title
        );
    }

    // 2. Materialize the revision plane (2 revisions) and one candidate per
    //    revision on the real in-memory engine.
    let store = Arc::new(SamyamaGraphStore::new());
    let v1 = seed_revision_plane_samyama(&store, 1, 3).await;
    let v2 = seed_revision_plane_samyama(&store, 2, 3).await;
    let materializer = FigureCandidateMaterializer::new(store.clone() as Arc<dyn DirectGraphStore>);
    let first = materializer
        .materialize(&candidate(&v1, 2))
        .await
        .expect("candidate v1");
    let second = materializer
        .materialize(&candidate(&v2, 2))
        .await
        .expect("candidate v2");
    assert!(first.applied && second.applied);
    assert!(!first.import_eligible && !second.import_eligible);

    // 3. Execute every task same-process; every one must PASS on the
    //    grounded graph.
    let mut results = Vec::new();
    for task in &tasks {
        let (verdict, rows, first_cell, error) = run_task(store.as_ref(), task).await;
        results.push((
            task.title.clone(),
            verdict.clone(),
            rows,
            first_cell,
            error.clone(),
        ));
        assert_eq!(
            verdict,
            Verdict::Pass,
            "task {:?} did not pass on grounded graph: rows={rows} first={first_cell:?} error={error:?}\ncypher: {}",
            task.title,
            task.cypher
        );
    }

    // 4. Durable task report: persisted JSON preserves D127 false.
    let report = serde_json::json!({
        "schema_version": "figure-candidate-graph-task.v1",
        "source": "data/graph_tasks/figure_candidate.tasks.cypher",
        "candidates": 2,
        "import_eligible": false,
        "failed": false,
        "tasks": results.iter().map(|(title, verdict, rows, first_cell, _)| serde_json::json!({
            "title": title,
            "verdict": format!("{verdict:?}").to_uppercase(),
            "rows": rows,
            "first_cell_number": first_cell,
        })).collect::<Vec<_>>(),
    });
    let dir = tempfile::tempdir().expect("tempdir for durable report");
    let report_path = dir.path().join("figure_candidate_report.json");
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap())
        .expect("write durable report");
    let persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("read durable report"))
            .expect("parse durable report");
    assert_eq!(persisted["import_eligible"], serde_json::json!(false));
    assert_eq!(persisted["failed"], serde_json::json!(false));
    assert_eq!(persisted["candidates"], serde_json::json!(2));
    assert_eq!(
        persisted["tasks"].as_array().map(|rows| rows.len()),
        Some(tasks.len())
    );

    // 5. Negative: an empty graph has no revision/section identity, so the
    //    grounded-lineage and lock-presence tasks must fail (EMPTY).
    let empty = Arc::new(SamyamaGraphStore::new());
    let mut empty_passes = 0;
    for task in &tasks {
        let (verdict, _, _, _) = run_task(empty.as_ref(), task).await;
        if verdict == Verdict::Pass {
            empty_passes += 1;
        }
    }
    assert!(
        empty_passes < tasks.len(),
        "suite must fail on a graph with no revision/section identity"
    );

    // 6. Negative: an orphan candidate whose revision/section identity points
    //    at nodes that do not exist breaks the lineage tasks. The materializer
    //    refuses to write such a node, so this simulates drifted external
    //    evidence directly on the store.
    let ghost = Arc::new(SamyamaGraphStore::new());
    let node = ghost.create_node("FigureCandidate").await.unwrap();
    ghost
        .set_node_property_string(node, "vid", "vid:candidate:ghost".into())
        .await
        .unwrap();
    ghost
        .set_node_property_string(
            node,
            "revision_vid",
            "vid:revision:arxiv:2507.19457:v9".into(),
        )
        .await
        .unwrap();
    ghost
        .set_node_property_string(node, "section_vid", "vid:section:ghost:9".into())
        .await
        .unwrap();
    ghost
        .set_node_property_bool(node, "import_eligible", false)
        .await
        .unwrap();
    ghost
        .set_node_property_bool(node, "authoritative", false)
        .await
        .unwrap();
    let mut lineage_failed = false;
    for task in &tasks {
        if task.title.contains("Grounded figure candidates")
            || task.title.contains("revision identity")
            || task.title.contains("section identity")
        {
            let (verdict, _, _, _) = run_task(ghost.as_ref(), task).await;
            if verdict != Verdict::Pass {
                lineage_failed = true;
            }
        }
    }
    assert!(
        lineage_failed,
        "grounded-lineage tasks must fail for a candidate with missing revision/section identity"
    );
}
