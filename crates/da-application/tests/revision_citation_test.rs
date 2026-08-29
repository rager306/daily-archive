//! PIR-011 closure proof (T04): a same-process two-revision MockGraphStore
//! query shows v1 and v5 keep disjoint Section VIDs, distinct CITES/HAS_PART
//! bibliography sets, and revision-local anchors — read back through
//! `ArticleRevisionView.reference_vids` / `.citation_arxiv_ids` (T02 surface)
//! plus the raw revision-owned edge properties (T03 anchors).
//!
//! The proof exercises the real ingest boundary (`materialize_parsed_with_context`
//! with a `RevisionContentContext`), not hand-built fixtures: both revisions
//! share one globally deduped Citation/Reference pair (MEM471) while each
//! keeps its own additive PaperRevision edges and in-text anchor payloads.

mod common;

use std::sync::Arc;

use common::mock_graph_store::MockGraphStore;
use da_application::ingest::IngestUseCase;
use da_application::revision_materialization::{
    RevisionContentContext, RevisionManifestationInput,
};
use da_application::revision_query::{RevisionQueryUseCase, RevisionSelector};
use da_domain::{paper_revision_vid, revision_section_vid, vid};
use da_ports::clock::FixedClock;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{CitationAnchor, CitationEntry, ParsedArticle, Section};
use kg_storage::InstrumentedGraphStore;

const PAPER_ID: &str = "2401.00042";

fn sha(seed: u64) -> String {
    format!("{seed:064x}")
}

/// Checksum-bound revision context: the manifestation sha256 must equal the
/// parsed pdf_hash (checksum binding, M397 S04) and the revision number picks
/// the content-addressed PaperRevision/Section identity.
fn revision_context(revision: u32, pdf_hash: &str) -> RevisionContentContext {
    RevisionContentContext {
        source_code: "arxiv".into(),
        canonical_id: PAPER_ID.into(),
        revision,
        manifestations: vec![RevisionManifestationInput {
            format: "pdf".into(),
            sha256: pdf_hash.to_string(),
            bytes: 1000 + u64::from(revision),
            uri: format!("https://arxiv.org/pdf/{PAPER_ID}v{revision}"),
            path: format!("{PAPER_ID}v{revision}.pdf"),
            capture_status: "captured".into(),
        }],
    }
}

fn parsed_article(
    pdf_hash: String,
    sections: Vec<Section>,
    citations: Vec<CitationEntry>,
) -> ParsedArticle {
    ParsedArticle {
        paper_id: PAPER_ID.into(),
        title: "Two-revision citation paper".into(),
        abstract_text: "Abstract shared across revisions".into(),
        body_text: "x".repeat(1000),
        sections,
        citations,
        layout_json: None,
        tei_xml: None,
        pdf_hash,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
        ..Default::default()
    }
}

/// v1: 2 sections, bibliography {c1 resolvable, c2 reference-only}.
/// c1 and c2 both carry in-text anchors.
fn v1_parsed() -> ParsedArticle {
    parsed_article(
        sha(1),
        vec![
            Section {
                title: "Introduction".into(),
                text: "intro v1 text".into(),
                level: 1,
                ..Default::default()
            },
            Section {
                title: "Methods".into(),
                text: "methods v1 text".into(),
                level: 1,
                ..Default::default()
            },
        ],
        vec![
            CitationEntry {
                raw_text: "Smith et al 2023".into(),
                doi: Some("10.1000/smith".into()),
                arxiv_id: Some("2301.00001".into()),
                title: Some("Cited Paper A".into()),
                // Multi-span: cited twice in v1 (sections "1" and "2").
                anchors: vec![
                    CitationAnchor {
                        ref_id: "b0".into(),
                        section_path_id: Some("1".into()),
                        quote: "[1]".into(),
                        offset: Some(0),
                    },
                    CitationAnchor {
                        ref_id: "b0".into(),
                        section_path_id: Some("2".into()),
                        quote: "[1]".into(),
                        offset: Some(12),
                    },
                ],
            },
            CitationEntry {
                raw_text: "Old v1 reference only".into(),
                doi: None,
                arxiv_id: None,
                title: None,
                anchors: vec![CitationAnchor {
                    ref_id: "b1".into(),
                    section_path_id: Some("1".into()),
                    quote: "[2]".into(),
                    offset: Some(3),
                }],
            },
        ],
    )
}

/// v5: 3 sections (Results added), bibliography {c1 shared with v1, c3 new
/// resolvable, c4 new reference-only}. c1 keeps the SAME raw_text (same
/// globally deduped Reference/Citation nodes, MEM471) but its anchor span
/// differs from v1 — the revision-owned edge must not overwrite v1's anchors.
fn v5_parsed() -> ParsedArticle {
    parsed_article(
        sha(5),
        vec![
            Section {
                title: "Introduction".into(),
                text: "intro v5 text".into(),
                level: 1,
                ..Default::default()
            },
            Section {
                title: "Methods".into(),
                text: "methods v5 text".into(),
                level: 1,
                ..Default::default()
            },
            Section {
                title: "Results".into(),
                text: "results v5 text".into(),
                level: 1,
                ..Default::default()
            },
        ],
        vec![
            CitationEntry {
                raw_text: "Smith et al 2023".into(), // shared raw_text → shared Reference/Citation
                doi: Some("10.1000/smith".into()),
                arxiv_id: Some("2301.00001".into()),
                title: Some("Cited Paper A".into()),
                // Different span than v1: offset 5, single occurrence.
                anchors: vec![CitationAnchor {
                    ref_id: "b0".into(),
                    section_path_id: Some("1".into()),
                    quote: "[1]".into(),
                    offset: Some(5),
                }],
            },
            CitationEntry {
                raw_text: "New v5 results reference".into(),
                doi: None,
                arxiv_id: Some("2301.00005".into()),
                title: Some("Cited Paper B".into()),
                // Anchor exists but its section path could not be resolved →
                // typed missingness anchor_section_path_unresolved (D249).
                anchors: vec![CitationAnchor {
                    ref_id: "b2".into(),
                    section_path_id: None,
                    quote: "[3]".into(),
                    offset: None,
                }],
            },
            CitationEntry {
                raw_text: "Reference-only v5 entry".into(),
                doi: None,
                arxiv_id: None,
                title: None,
                anchors: vec![],
            },
        ],
    )
}

async fn materialize(ingest: &IngestUseCase, parsed: &ParsedArticle, revision: u32) {
    ingest
        .materialize_parsed_with_context(
            parsed,
            vec![0.1; 1024],
            None,
            Some("arxiv".into()),
            None,
            da_domain::PdfTriageStatus::NotConfigured,
            Some(&revision_context(revision, &parsed.pdf_hash)),
        )
        .await
        .expect("revision materialization must succeed");
}

fn anchor_ids(anchors: &[CitationAnchor]) -> Vec<(String, Option<String>, String, Option<u32>)> {
    anchors
        .iter()
        .map(|a| {
            (
                a.ref_id.clone(),
                a.section_path_id.clone(),
                a.quote.clone(),
                a.offset,
            )
        })
        .collect()
}

#[tokio::test]
async fn two_revision_citation_ownership_is_disjoint_and_readable() {
    let inner = MockGraphStore::new();
    let instrumented = InstrumentedGraphStore::new(Arc::new(inner.clone()));
    let ingest = IngestUseCase::materializer_only_with_clock(
        Arc::new(instrumented.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );

    materialize(&ingest, &v1_parsed(), 1).await;
    materialize(&ingest, &v5_parsed(), 5).await;

    // ---- Graph shape: one Paper, two PaperRevisions, no node cloning ----
    assert_eq!(inner.get_nodes_by_label("Paper").await.len(), 1);
    let revisions = inner.get_nodes_by_label("PaperRevision").await;
    assert_eq!(revisions.len(), 2, "one PaperRevision per source revision");
    let references = inner.get_nodes_by_label("Reference").await;
    assert_eq!(
        references.len(),
        4,
        "references stay globally deduped per raw_text (MEM471)"
    );
    let citations = inner.get_nodes_by_label("Citation").await;
    assert_eq!(
        citations.len(),
        2,
        "only resolvable arxiv_ids get Citation nodes, deduped globally (MEM471)"
    );

    // ---- Query surface: ArticleRevisionView exposes the citation identity ----
    let query = RevisionQueryUseCase::new(Arc::new(inner.clone()));
    let response = query
        .query(PAPER_ID, RevisionSelector::History)
        .await
        .unwrap();
    assert_eq!(response.revisions.len(), 2);
    let v1 = &response.revisions[0];
    let v5 = &response.revisions[1];
    assert_eq!((v1.revision, v5.revision), (1, 5));

    // D248: section_count counts only Section-labeled hasPart targets; the
    // bibliography Reference targets must never masquerade as sections.
    assert_eq!(v1.section_count, 2);
    assert_eq!(v5.section_count, 3);

    // Distinct HAS_PART bibliography sets per revision: v1 owns its two
    // references, v5 owns its three; the shared raw_text lands in both sets
    // through the SAME globally deduped Reference node (no clone, D246).
    let shared_ref = vid::reference_vid("Smith et al 2023");
    assert_eq!(v1.reference_vids.len(), 2);
    assert_eq!(v5.reference_vids.len(), 3);
    assert!(v1.reference_vids.contains(&shared_ref));
    assert!(v5.reference_vids.contains(&shared_ref));
    assert_ne!(
        v1.reference_vids, v5.reference_vids,
        "revision bibliography sets must differ"
    );

    // Distinct CITES membership per revision.
    assert_eq!(v1.citation_arxiv_ids, ["2301.00001"]);
    assert_eq!(v5.citation_arxiv_ids, ["2301.00001", "2301.00005"]);
    assert_ne!(
        v1.citation_arxiv_ids, v5.citation_arxiv_ids,
        "revision CITES sets must differ"
    );

    // ---- Disjoint Section VIDs: content-addressed identity by construction,
    // verified against the nodes actually present in the graph ----
    let v1_rev_vid = paper_revision_vid("arxiv", PAPER_ID, 1).unwrap();
    let v5_rev_vid = paper_revision_vid("arxiv", PAPER_ID, 5).unwrap();
    assert_ne!(v1_rev_vid, v5_rev_vid);
    let v1_section_vids: Vec<String> = (0..2)
        .map(|ordinal| revision_section_vid(&v1_rev_vid, ordinal).unwrap())
        .collect();
    let v5_section_vids: Vec<String> = (0..3)
        .map(|ordinal| revision_section_vid(&v5_rev_vid, ordinal).unwrap())
        .collect();
    assert!(
        v1_section_vids.iter().all(|v| !v5_section_vids.contains(v)),
        "v1 and v5 Section VIDs must be disjoint"
    );
    for expected in v1_section_vids.iter().chain(v5_section_vids.iter()) {
        assert!(
            inner
                .find_node_by_string_property("Section", "vid", expected)
                .await
                .is_some(),
            "expected revision-scoped Section {expected} must exist in the graph"
        );
    }
    // Every Section node in the graph belongs to exactly one revision.
    let all_section_vids: Vec<String> = {
        let mut vids: Vec<String> = Vec::new();
        for section in inner.get_nodes_by_label("Section").await {
            vids.push(
                inner
                    .get_node_property_string(section, "vid")
                    .await
                    .expect("Section must carry its vid"),
            );
        }
        vids
    };
    assert_eq!(all_section_vids.len(), 5);
    assert!(
        all_section_vids
            .iter()
            .all(|v| v1_section_vids.contains(v) || v5_section_vids.contains(v)),
        "no versionless or cross-revision Section VID may exist"
    );
}

#[tokio::test]
async fn anchors_are_revision_local_on_shared_citation() {
    let inner = MockGraphStore::new();
    let instrumented = InstrumentedGraphStore::new(Arc::new(inner.clone()));
    let ingest = IngestUseCase::materializer_only_with_clock(
        Arc::new(instrumented.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );

    materialize(&ingest, &v1_parsed(), 1).await;
    materialize(&ingest, &v5_parsed(), 5).await;

    let rev1 = inner
        .find_node_by_string_property(
            "PaperRevision",
            "vid",
            &paper_revision_vid("arxiv", PAPER_ID, 1).unwrap(),
        )
        .await
        .expect("v1 revision node");
    let rev5 = inner
        .find_node_by_string_property(
            "PaperRevision",
            "vid",
            &paper_revision_vid("arxiv", PAPER_ID, 5).unwrap(),
        )
        .await
        .expect("v5 revision node");
    let paper = inner.get_nodes_by_label("Paper").await[0];
    let cited_a = inner
        .find_node_by_string_property("Citation", "arxiv_id", "2301.00001")
        .await
        .expect("globally deduped Citation 2301.00001");

    // Same Citation node, two revision-owned CITES edges, different anchors.
    let (v1_cites_eid, _) = inner.get_edges_between(rev1, cited_a, "CITES").await[0];
    let v1_anchors: Vec<CitationAnchor> = serde_json::from_str(
        &inner
            .get_edge_property_string(v1_cites_eid, "citation_anchors_json")
            .await
            .expect("v1 CITES edge must carry anchors"),
    )
    .unwrap();
    assert_eq!(
        anchor_ids(&v1_anchors),
        [
            ("b0".into(), Some("1".into()), "[1]".into(), Some(0)),
            ("b0".into(), Some("2".into()), "[1]".into(), Some(12)),
        ]
    );
    assert_eq!(
        inner
            .get_edge_property_string(v1_cites_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some(""),
        "v1 anchors resolved → empty missingness"
    );

    let (v5_cites_eid, _) = inner.get_edges_between(rev5, cited_a, "CITES").await[0];
    let v5_anchors: Vec<CitationAnchor> = serde_json::from_str(
        &inner
            .get_edge_property_string(v5_cites_eid, "citation_anchors_json")
            .await
            .expect("v5 CITES edge must carry its own anchors"),
    )
    .unwrap();
    assert_eq!(
        anchor_ids(&v5_anchors),
        [("b0".into(), Some("1".into()), "[1]".into(), Some(5))]
    );
    assert_ne!(
        v1_anchors, v5_anchors,
        "shared Citation keeps revision-local anchor spans — v5 must not overwrite v1"
    );

    // Global dedup surface stays anchor-free (MEM796 / D247): the Citation
    // node and the Paper-sourced edge never carry anchor properties.
    assert_eq!(
        inner
            .get_node_property_string(cited_a, "citation_anchors_json")
            .await,
        None,
        "global Citation node must never carry anchors"
    );
    let (paper_cites_eid, _) = inner.get_edges_between(paper, cited_a, "CITES").await[0];
    assert_eq!(
        inner
            .get_edge_property_string(paper_cites_eid, "citation_anchors_json")
            .await,
        None,
        "Paper-sourced CITES edge must never carry anchors"
    );

    // Reference-only v1 entry: anchors on the revision HAS_PART edge (D249).
    let v1_ref_only = inner
        .find_node_by_string_property(
            "Reference",
            "vid",
            &vid::reference_vid("Old v1 reference only"),
        )
        .await
        .expect("v1 reference-only Reference node");
    let (v1_hp_eid, _) = inner.get_edges_between(rev1, v1_ref_only, "hasPart").await[0];
    let v1_hp_anchors: Vec<CitationAnchor> = serde_json::from_str(
        &inner
            .get_edge_property_string(v1_hp_eid, "citation_anchors_json")
            .await
            .expect("v1 reference-only HAS_PART edge must carry anchors"),
    )
    .unwrap();
    assert_eq!(
        anchor_ids(&v1_hp_anchors),
        [("b1".into(), Some("1".into()), "[2]".into(), Some(3))]
    );

    // Typed missingness on v5 edges: unresolved section path for c3 and
    // no_in_text_anchor for the anchor-less reference-only c4.
    let cited_b = inner
        .find_node_by_string_property("Citation", "arxiv_id", "2301.00005")
        .await
        .expect("Citation 2301.00005");
    let (v5_cites_b_eid, _) = inner.get_edges_between(rev5, cited_b, "CITES").await[0];
    assert_eq!(
        inner
            .get_edge_property_string(v5_cites_b_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some("anchor_section_path_unresolved")
    );
    let v5_ref_only = inner
        .find_node_by_string_property(
            "Reference",
            "vid",
            &vid::reference_vid("Reference-only v5 entry"),
        )
        .await
        .expect("v5 reference-only Reference node");
    let (v5_hp_eid, _) = inner.get_edges_between(rev5, v5_ref_only, "hasPart").await[0];
    assert_eq!(
        inner
            .get_edge_property_string(v5_hp_eid, "citation_anchors_json")
            .await
            .as_deref(),
        Some("[]")
    );
    assert_eq!(
        inner
            .get_edge_property_string(v5_hp_eid, "citation_anchor_missingness")
            .await
            .as_deref(),
        Some("no_in_text_anchor")
    );
}

#[tokio::test]
async fn replay_stays_applied_zero_and_d127_holds() {
    let inner = MockGraphStore::new();
    let instrumented = InstrumentedGraphStore::new(Arc::new(inner.clone()));
    let ingest = IngestUseCase::materializer_only_with_clock(
        Arc::new(instrumented.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );

    let v1 = v1_parsed();
    let v5 = v5_parsed();
    materialize(&ingest, &v1, 1).await;
    materialize(&ingest, &v5, 5).await;
    assert!(
        instrumented.reset().await.applied() > 0,
        "first two-revision materialization must apply writes"
    );
    let nodes_before = inner.node_count_total();
    let edges_before = inner.edge_count_total();

    // Replay the identical two-revision sequence: applied=0, unchanged>0.
    materialize(&ingest, &v1, 1).await;
    materialize(&ingest, &v5, 5).await;
    let replay = instrumented.snapshot().await;
    assert!(replay.partition_is_valid());
    assert!(replay.attempted() > 0);
    assert_eq!(
        replay.applied(),
        0,
        "identical two-revision replay must not change graph state"
    );
    assert_eq!(replay.rejected(), 0);
    assert!(replay.unchanged() > 0);
    assert_eq!(inner.node_count_total(), nodes_before);
    assert_eq!(inner.edge_count_total(), edges_before);

    // D127: no node may ever carry import_eligible=true; the revision-owned
    // node classes must all carry the explicit false lock.
    let locked_classes = ["Paper", "PaperRevision", "Section", "Reference", "Citation"];
    for label in locked_classes {
        for node in inner.get_nodes_by_label(label).await {
            assert_eq!(
                inner.get_node_property_bool(node, "import_eligible").await,
                Some(false),
                "D127: {label} node must stay import_eligible=false"
            );
        }
    }
}

#[tokio::test]
async fn legacy_wire_bounds_decode_on_revision_edges() {
    // PIR-008 (M398/S04/T03): the revision as-of surface decodes edge
    // `invalid_at` through the bounded compatibility decoder. A legacy
    // integer Unix-seconds bound deactivates an edge; the OPEN sentinel keeps
    // it active even when spelled as a string property. Raw string-presence
    // checks would get both wrong (OPEN string present → inactive; integer
    // wire → indistinguishable from RFC3339 without parsing).
    let inner = MockGraphStore::new();
    let instrumented = InstrumentedGraphStore::new(Arc::new(inner.clone()));
    let ingest = IngestUseCase::materializer_only_with_clock(
        Arc::new(instrumented.clone()),
        Arc::new(FixedClock::from_timestamp(1_700_000_000)),
    );
    materialize(&ingest, &v1_parsed(), 1).await;
    materialize(&ingest, &v5_parsed(), 5).await;
    let query = RevisionQueryUseCase::new(Arc::new(inner.clone()));

    let rev1 = inner
        .find_node_by_string_property(
            "PaperRevision",
            "vid",
            &paper_revision_vid("arxiv", PAPER_ID, 1).unwrap(),
        )
        .await
        .expect("v1 revision node");
    let rev5 = inner
        .find_node_by_string_property(
            "PaperRevision",
            "vid",
            &paper_revision_vid("arxiv", PAPER_ID, 5).unwrap(),
        )
        .await
        .expect("v5 revision node");
    let cited_a = inner
        .find_node_by_string_property("Citation", "arxiv_id", "2301.00001")
        .await
        .expect("globally deduped Citation 2301.00001");

    // Baseline: v1 owns 2301.00001 through an open (no invalid_at) CITES edge.
    let exact_v1 = query
        .query(PAPER_ID, RevisionSelector::Exact(1))
        .await
        .unwrap();
    assert_eq!(exact_v1.revisions[0].citation_arxiv_ids, ["2301.00001"]);

    // Legacy integer invalid_at (integer-seconds string wire) deactivates the
    // v1 CITES edge → the citation leaves the revision's owned set.
    let (v1_cites_eid, _) = inner.get_edges_between(rev1, cited_a, "CITES").await[0];
    inner
        .set_edge_property_string(v1_cites_eid, "invalid_at", "1600000000")
        .await
        .unwrap();
    let exact_v1 = query
        .query(PAPER_ID, RevisionSelector::Exact(1))
        .await
        .unwrap();
    assert!(
        exact_v1.revisions[0].citation_arxiv_ids.is_empty(),
        "legacy integer invalid_at must deactivate the revision-owned CITES edge"
    );

    // OPEN invalid_at keeps the v5→v1 SUPERSEDES edge active: the predecessor
    // must still resolve even when the open bound is spelled as a string.
    let (v5_sup_eid, _) = inner.get_edges_between(rev5, rev1, "SUPERSEDES").await[0];
    inner
        .set_edge_property_string(v5_sup_eid, "invalid_at", kg_ontology::temporal::OPEN)
        .await
        .unwrap();
    let exact_v5 = query
        .query(PAPER_ID, RevisionSelector::Exact(5))
        .await
        .unwrap();
    assert_eq!(
        exact_v5.revisions[0].predecessor_revision,
        Some(1),
        "OPEN invalid_at is an open bound — the SUPERSEDES predecessor edge stays active"
    );
}
