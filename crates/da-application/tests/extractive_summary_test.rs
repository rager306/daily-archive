//! ADR-051 suite for revision-scoped grounded extractive summaries (M363/S01).

use std::sync::Arc;

use da_application::extractive_summary::{
    ExtractiveSummaryMaterializeError, ExtractiveSummaryMaterializeInput, ExtractiveSummaryUseCase,
    SectionSource,
};
use da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD;
use da_domain::relation::structure::HAS_SUMMARY;
use da_domain::vid::{paper_revision_vid, revision_section_vid, revision_summary_vid};
use da_ports::DirectGraphStore;
use kg_storage::MockGraphStore;

async fn seed_revision(
    store: &MockGraphStore,
    revision: u32,
    method_text: &str,
) -> (u64, String, Vec<SectionSource>) {
    let revision_vid = paper_revision_vid("arxiv", "2507.19457", revision).unwrap();
    let revision_node = store.create_node("PaperRevision").await.unwrap();
    store
        .set_node_property_string(revision_node, "vid", revision_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_int(revision_node, "revision", revision as i64)
        .await
        .unwrap();
    store
        .set_node_property_bool(revision_node, "import_eligible", false)
        .await
        .unwrap();

    let mut sections = Vec::new();
    for (ordinal, path, title, text) in [
        (
            0,
            "1",
            "Introduction",
            "We study prompt optimization with reflective evolution.",
        ),
        (1, "2", "Method", method_text),
        (2, "3", "Empty", "   "),
    ] {
        let source_vid = revision_section_vid(&revision_vid, ordinal).unwrap();
        let node = store.create_node("Section").await.unwrap();
        store
            .set_node_property_string(node, "vid", source_vid.clone())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "path_id", path.into())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "title", title.into())
            .await
            .unwrap();
        store
            .set_node_property_string(node, "text", text.into())
            .await
            .unwrap();
        sections.push(SectionSource {
            path_id: path.into(),
            source_vid,
            title: title.into(),
            text: text.into(),
            node_id: node,
        });
    }
    (revision_node, revision_vid, sections)
}

fn input(
    revision_node_id: u64,
    revision_vid: String,
    sections: Vec<SectionSource>,
) -> ExtractiveSummaryMaterializeInput {
    ExtractiveSummaryMaterializeInput {
        paper_id: "2507.19457".into(),
        revision_node_id,
        revision_vid,
        abstract_text: "GEPA evolves prompts. Results improve by 12%.".into(),
        sections,
        max_chars: 160,
    }
}

#[tokio::test]
async fn materializes_revision_abstract_and_section_summaries() {
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts with Pareto selection.").await;
    let result = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .materialize(input(revision_node, revision_vid.clone(), sections.clone()))
        .await
        .unwrap();

    assert!(result.abstract_written);
    assert_eq!(result.section_summaries_written, 2);
    assert_eq!(result.skipped_empty_sections, 1);
    assert_eq!(result.method, EXTRACTIVE_SUMMARY_METHOD);

    let abstract_vid = revision_summary_vid(&revision_vid, "abstract", &revision_vid).unwrap();
    let abstract_node = store
        .find_node_by_string_property("Summary", "vid", &abstract_vid)
        .await
        .expect("revision abstract summary");
    assert_eq!(
        store
            .get_node_property_string(abstract_node, "revision_vid")
            .await
            .as_deref(),
        Some(revision_vid.as_str())
    );
    assert_eq!(
        store
            .get_node_property_string(abstract_node, "source_id")
            .await
            .as_deref(),
        Some(revision_vid.as_str())
    );
    assert_eq!(
        store
            .get_edges_between(revision_node, abstract_node, HAS_SUMMARY)
            .await
            .len(),
        1
    );

    let section_vid = sections[0].source_vid.clone();
    let section_summary_vid = revision_summary_vid(&revision_vid, "section", &section_vid).unwrap();
    let section_summary = store
        .find_node_by_string_property("Summary", "vid", &section_summary_vid)
        .await
        .expect("revision section summary");
    assert_eq!(
        store
            .get_node_property_string(section_summary, "source_id")
            .await
            .as_deref(),
        Some(section_vid.as_str())
    );
    assert_eq!(
        store
            .get_edges_between(sections[0].node_id, section_summary, HAS_SUMMARY)
            .await
            .len(),
        1
    );
    assert!(
        !store
            .get_node_property_bool(section_summary, "import_eligible")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn two_revisions_with_same_paths_keep_distinct_summary_history() {
    let store = Arc::new(MockGraphStore::new());
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>);
    let (v1_node, v1_vid, v1_sections) =
        seed_revision(&store, 1, "Version one uses beam search.").await;
    let (v5_node, v5_vid, v5_sections) =
        seed_revision(&store, 5, "Version five uses Pareto search.").await;

    use_case
        .materialize(input(v1_node, v1_vid.clone(), v1_sections.clone()))
        .await
        .unwrap();
    use_case
        .materialize(input(v5_node, v5_vid.clone(), v5_sections.clone()))
        .await
        .unwrap();

    let v1_summary_vid =
        revision_summary_vid(&v1_vid, "section", &v1_sections[1].source_vid).unwrap();
    let v5_summary_vid =
        revision_summary_vid(&v5_vid, "section", &v5_sections[1].source_vid).unwrap();
    assert_ne!(v1_summary_vid, v5_summary_vid);
    let v1_summary = store
        .find_node_by_string_property("Summary", "vid", &v1_summary_vid)
        .await
        .unwrap();
    let v5_summary = store
        .find_node_by_string_property("Summary", "vid", &v5_summary_vid)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_node_property_string(v1_summary, "text")
            .await
            .as_deref(),
        Some("Version one uses beam search.")
    );
    assert_eq!(
        store
            .get_node_property_string(v5_summary, "text")
            .await
            .as_deref(),
        Some("Version five uses Pareto search.")
    );
}

#[tokio::test]
async fn materialization_is_idempotent() {
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>);
    let materialize_input = input(revision_node, revision_vid.clone(), sections);
    let first = use_case
        .materialize(materialize_input.clone())
        .await
        .unwrap();
    let second = use_case.materialize(materialize_input).await.unwrap();
    assert_eq!(
        first.section_summaries_written,
        second.section_summaries_written
    );
    let abstract_vid = revision_summary_vid(&revision_vid, "abstract", &revision_vid).unwrap();
    let abstract_node = store
        .find_node_by_string_property("Summary", "vid", &abstract_vid)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_edges_between(revision_node, abstract_node, HAS_SUMMARY)
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn missing_revision_node_fails_closed() {
    let store = Arc::new(MockGraphStore::new());
    let revision_vid = paper_revision_vid("arxiv", "2507.19457", 1).unwrap();
    let source_vid = revision_section_vid(&revision_vid, 0).unwrap();
    let use_case = ExtractiveSummaryUseCase::new(store as Arc<dyn DirectGraphStore>);
    let error = use_case
        .materialize(input(
            999_998,
            revision_vid,
            vec![SectionSource {
                path_id: "1".into(),
                source_vid,
                title: "Method".into(),
                text: "Body".into(),
                node_id: 999_999,
            }],
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ExtractiveSummaryMaterializeError::MissingRevisionNode(_)
    ));
}

#[tokio::test]
async fn missing_section_node_fails_closed_before_any_summary_write() {
    let store = Arc::new(MockGraphStore::new());
    let revision_vid = paper_revision_vid("arxiv", "2507.19457", 1).unwrap();
    let revision_node = store.create_node("PaperRevision").await.unwrap();
    store
        .set_node_property_string(revision_node, "vid", revision_vid.clone())
        .await
        .unwrap();
    let source_vid = revision_section_vid(&revision_vid, 0).unwrap();
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>);
    let error = use_case
        .materialize(input(
            revision_node,
            revision_vid,
            vec![SectionSource {
                path_id: "1".into(),
                source_vid: source_vid.clone(),
                title: "Method".into(),
                text: "Body".into(),
                node_id: 999_999,
            }],
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ExtractiveSummaryMaterializeError::MissingSectionNode(ref vid) if vid == &source_vid
    ));
    assert!(store.get_nodes_by_label("Summary").await.is_empty());
}
