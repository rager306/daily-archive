//! ADR-051 suite for revision-scoped grounded extractive summaries (M363/S01).

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use da_application::extractive_summary::{
    ExtractiveSummaryMaterializeError, ExtractiveSummaryMaterializeInput, ExtractiveSummaryUseCase,
    SectionRolePolicyBinding, SectionSource,
};
use da_domain::extractive_summary::EXTRACTIVE_SUMMARY_METHOD;
use da_domain::relation::structure::{HAS_PART, HAS_SUMMARY};
use da_domain::section_summary_policy::{SectionRolePolicy, SectionRoleRule, SummaryType};
use da_domain::vid::{paper_revision_vid, revision_section_vid, revision_summary_vid};
use da_ports::DirectGraphStore;
use da_ports::clock::{Clock, FixedClock};
use kg_storage::MockGraphStore;

/// Deterministic mutable wall clock (ADR-051 Contour 1): tests set the instant
/// between materialize runs so version intervals can be pinned exactly
/// (old.valid_to == new.valid_from).
#[derive(Clone, Default)]
struct MutableClock(Arc<Mutex<DateTime<Utc>>>);

impl MutableClock {
    fn new(at: DateTime<Utc>) -> Self {
        Self(Arc::new(Mutex::new(at)))
    }

    fn set(&self, at: DateTime<Utc>) {
        *self.0.lock().unwrap() = at;
    }
}

impl Clock for MutableClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

fn ts(secs: i64) -> DateTime<Utc> {
    FixedClock::from_timestamp(secs).now()
}

/// All Summary nodes belonging to one logical group (D243): nodes whose
/// `logical_vid` matches, or legacy pre-T03 nodes that carry the logical VID
/// directly in `vid` without a `logical_vid` property.
async fn summary_group_nodes(store: &MockGraphStore, logical_vid: &str) -> Vec<u64> {
    let mut members = Vec::new();
    for node in store.get_nodes_by_label("Summary").await {
        let logical = store.get_node_property_string(node, "logical_vid").await;
        let vid = store.get_node_property_string(node, "vid").await;
        if logical.as_deref() == Some(logical_vid)
            || (logical.is_none() && vid.as_deref() == Some(logical_vid))
        {
            members.push(node);
        }
    }
    members
}

/// The current (not superseded) Summary node of a logical group. Exactly one
/// node per group is current after every write (PIR-012 invariant).
async fn current_summary_node(store: &MockGraphStore, logical_vid: &str) -> Option<u64> {
    for node in summary_group_nodes(store, logical_vid).await {
        let superseded_at = store.get_node_property_string(node, "superseded_at").await;
        if superseded_at.as_deref().is_none_or(|s| s.is_empty()) {
            return Some(node);
        }
    }
    None
}

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
            path_id_missingness: String::new(),
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
    let abstract_node = current_summary_node(&store, &abstract_vid)
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
    let section_summary = current_summary_node(&store, &section_summary_vid)
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
            .get_node_property_string(section_summary, "summary_type")
            .await
            .as_deref(),
        Some("generic")
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
async fn summary_node_persists_source_path_id() {
    // PIR-007 (T02): Summary nodes carry a durable dotted source_path_id so
    // downstream joins and S05 honesty resolve the source Section by AST path
    // instead of an invented ordinal.
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, mut sections) =
        seed_revision(&store, 1, "GEPA mutates prompts with Pareto selection.").await;
    // Nested section: materialize must persist the dotted path, not an ordinal.
    sections[1].path_id = "3.2.1".into();
    ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .materialize(input(revision_node, revision_vid.clone(), sections.clone()))
        .await
        .unwrap();

    let abstract_vid = revision_summary_vid(&revision_vid, "abstract", &revision_vid).unwrap();
    let abstract_node = current_summary_node(&store, &abstract_vid)
        .await
        .expect("revision abstract summary");
    assert_eq!(
        store
            .get_node_property_string(abstract_node, "source_path_id")
            .await
            .as_deref(),
        Some("abstract")
    );
    assert_eq!(
        store
            .get_node_property_string(abstract_node, "source_path_missingness")
            .await
            .as_deref(),
        Some(""),
        "abstract summary has no path missingness"
    );

    let summary_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let summary_node = current_summary_node(&store, &summary_vid)
        .await
        .expect("nested section summary");
    assert_eq!(
        store
            .get_node_property_string(summary_node, "source_path_id")
            .await
            .as_deref(),
        Some("3.2.1"),
        "nested path must survive materialization, not collapse to an ordinal"
    );
    assert_eq!(
        store
            .get_node_property_string(summary_node, "source_path_missingness")
            .await
            .as_deref(),
        Some(""),
        "real dotted path is not a fallback"
    );
}

#[tokio::test]
async fn materialize_persists_typed_path_fallback_missingness() {
    // PIR-007 (T02): when the walker had to derive path_id from `order`
    // because no real dotted AST path exists, the typed fallback reason is
    // mirrored onto the Summary node so honesty checks can tell invented
    // ordinals from real paths after materialization.
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, mut sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    // Simulate an order-only section: path_id came from the ordinal and the
    // collector recorded the typed missingness reason.
    sections[1].path_id = "9".into();
    sections[1].path_id_missingness = "path_id_missing_fallback_to_order".into();
    ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .materialize(input(revision_node, revision_vid.clone(), sections.clone()))
        .await
        .unwrap();

    let summary_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let summary_node = current_summary_node(&store, &summary_vid)
        .await
        .expect("order-fallback section summary");
    assert_eq!(
        store
            .get_node_property_string(summary_node, "source_path_id")
            .await
            .as_deref(),
        Some("9")
    );
    assert_eq!(
        store
            .get_node_property_string(summary_node, "source_path_missingness")
            .await
            .as_deref(),
        Some("path_id_missing_fallback_to_order")
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
    let v1_summary = current_summary_node(&store, &v1_summary_vid).await.unwrap();
    let v5_summary = current_summary_node(&store, &v5_summary_vid).await.unwrap();
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
async fn role_policy_specializes_matching_summaries_and_counts_unmatched() {
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, mut sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    sections[0].title = "Results".into();
    sections[1].title = "2. Methods".into();
    let policy = SectionRolePolicy {
        schema_version: "test.v1".into(),
        max_title_chars: 160,
        rules: vec![
            SectionRoleRule {
                summary_type: SummaryType::new("methods").unwrap(),
                allow_suffix_match: true,
                title_aliases: vec!["methods".into()],
            },
            SectionRoleRule {
                summary_type: SummaryType::new("results").unwrap(),
                allow_suffix_match: true,
                title_aliases: vec!["results".into()],
            },
        ],
    };
    let use_case = ExtractiveSummaryUseCase::with_role_policy(
        store.clone() as Arc<dyn DirectGraphStore>,
        SectionRolePolicyBinding {
            policy,
            configuration_sha256: "a".repeat(64),
        },
    )
    .unwrap();
    let result = use_case
        .materialize(input(revision_node, revision_vid.clone(), sections.clone()))
        .await
        .unwrap();
    assert_eq!(result.summary_type_counts.get("methods"), Some(&1));
    assert_eq!(result.summary_type_counts.get("results"), Some(&1));
    assert_eq!(result.unmatched_section_summaries, 0);

    let methods_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let methods_node = current_summary_node(&store, &methods_vid).await.unwrap();
    assert_eq!(
        store
            .get_node_property_string(methods_node, "summary_type")
            .await
            .as_deref(),
        Some("methods")
    );
    assert_eq!(
        store
            .get_node_property_string(methods_node, "role_policy_sha256")
            .await
            .as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    let results_vid =
        revision_summary_vid(&revision_vid, "section", &sections[0].source_vid).unwrap();
    let results_node = current_summary_node(&store, &results_vid).await.unwrap();
    assert_eq!(
        store
            .get_node_property_string(results_node, "summary_type")
            .await
            .as_deref(),
        Some("results")
    );

    // Replaying without specialization must NOT silently rewrite the
    // historical policy decision (PIR-012): the old version stays closed with
    // its "methods"/"results" metadata, and a NEW current version carries the
    // fresh policy digest (empty = generic).
    ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .materialize(input(revision_node, revision_vid, sections))
        .await
        .unwrap();
    // Old version: closed, historical policy decision preserved.
    assert!(
        store
            .get_node_property_string(methods_node, "superseded_at")
            .await
            .as_deref()
            .is_some_and(|s| !s.is_empty()),
        "methods version must be superseded after policy change"
    );
    assert_eq!(
        store
            .get_node_property_string(methods_node, "summary_type")
            .await
            .as_deref(),
        Some("methods"),
        "historical policy decision stays visible on the superseded version"
    );
    assert!(
        !store
            .get_node_property_bool(methods_node, "retrieval_eligible")
            .await
            .unwrap()
    );
    // New current version: generic, no policy digest, same logical group.
    let current_methods = current_summary_node(&store, &methods_vid).await.unwrap();
    assert_ne!(
        current_methods, methods_node,
        "policy change mints a new version"
    );
    assert_eq!(
        store
            .get_node_property_string(current_methods, "summary_type")
            .await
            .as_deref(),
        Some("generic")
    );
    assert_eq!(
        store
            .get_node_property_string(current_methods, "role_policy_sha256")
            .await
            .as_deref(),
        Some("")
    );
    assert_eq!(
        store
            .get_node_property_string(current_methods, "logical_vid")
            .await
            .as_deref(),
        Some(methods_vid.as_str()),
        "logical identity stays joinable across versions"
    );
    let current_results = current_summary_node(&store, &results_vid).await.unwrap();
    assert_ne!(current_results, results_node);
    assert_eq!(
        store
            .get_node_property_string(current_results, "summary_type")
            .await
            .as_deref(),
        Some("generic")
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
    let abstract_node = current_summary_node(&store, &abstract_vid).await.unwrap();
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
                path_id_missingness: String::new(),
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
                path_id_missingness: String::new(),
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

async fn seed_revision_with_parts(
    store: &MockGraphStore,
    paper_id: &str,
    revision: u32,
    method_text: &str,
) -> (u64, String) {
    let (revision_node, revision_vid, sections) = seed_revision(store, revision, method_text).await;
    store
        .set_node_property_string(revision_node, "canonical_id", paper_id.into())
        .await
        .unwrap();
    store
        .set_node_property_string(
            revision_node,
            "abstract_text",
            "GEPA evolves prompts.".into(),
        )
        .await
        .unwrap();
    for (ordinal, section) in sections.iter().enumerate() {
        store
            .set_node_property_int(section.node_id, "order", ordinal as i64)
            .await
            .unwrap();
        store
            .create_edge(revision_node, section.node_id, HAS_PART)
            .await
            .unwrap();
    }
    (revision_node, revision_vid)
}

#[tokio::test]
async fn walker_collects_two_revisions_and_skips_identity_less_nodes() {
    use da_application::extractive_summary::collect_revision_summary_inputs;
    let store = Arc::new(MockGraphStore::new());
    let (_, vid5) = seed_revision_with_parts(
        &store,
        "2507.19457",
        5,
        "GEPA mutates prompts with Pareto selection.",
    )
    .await;
    let (_, vid2) =
        seed_revision_with_parts(&store, "2506.07853", 2, "Second paper method text.").await;
    let orphan = store.create_node("PaperRevision").await.unwrap();
    store
        .set_node_property_string(orphan, "canonical_id", "no-vid".into())
        .await
        .unwrap();
    let walk = collect_revision_summary_inputs(store.as_ref() as &dyn DirectGraphStore, 160).await;
    assert_eq!(walk.inputs.len(), 2);
    assert_eq!(walk.inputs[0].revision_vid, vid2);
    assert_eq!(walk.inputs[1].revision_vid, vid5);
    assert_eq!(walk.inputs[1].paper_id, "2507.19457");
    assert_eq!(walk.inputs[1].sections.len(), 3);
    assert!(
        walk.inputs[1]
            .sections
            .iter()
            .any(|s| s.text.trim().is_empty())
    );
    assert_eq!(walk.skipped.len(), 1);
    assert_eq!(walk.skipped[0].reason, "missing_vid");
    assert_eq!(walk.skipped[0].node_id, orphan);
    // Seeded sections carry a real dotted path_id, so no fallback is needed.
    assert!(walk.path_id_fallbacks.is_empty());
}

#[tokio::test]
async fn walker_prefers_dotted_path_id_over_order_and_records_fallbacks() {
    use da_application::extractive_summary::collect_revision_summary_inputs;
    let store = Arc::new(MockGraphStore::new());
    let (revision_node, revision_vid, seed_sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    store
        .set_node_property_string(revision_node, "canonical_id", "2507.19457".into())
        .await
        .unwrap();
    // seed_revision returns sections without linking them; link them now.
    for section in &seed_sections {
        store
            .create_edge(revision_node, section.node_id, HAS_PART)
            .await
            .unwrap();
    }

    // Nested section: dotted path_id "3.2" with a conflicting order=2.
    // PIR-007: must be collected as "3.2", not as the order ordinal "2".
    let nested_vid = revision_section_vid(&revision_vid, 7).unwrap();
    let nested = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(nested, "vid", nested_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(nested, "path_id", "3.2".into())
        .await
        .unwrap();
    store
        .set_node_property_int(nested, "order", 2)
        .await
        .unwrap();
    store
        .set_node_property_string(nested, "title", "Nested".into())
        .await
        .unwrap();
    store
        .set_node_property_string(nested, "text", "Nested body text.".into())
        .await
        .unwrap();
    store
        .create_edge(revision_node, nested, HAS_PART)
        .await
        .unwrap();

    // Section with order only: typed fallback to the order ordinal.
    let order_only_vid = revision_section_vid(&revision_vid, 8).unwrap();
    let order_only = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(order_only, "vid", order_only_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_int(order_only, "order", 9)
        .await
        .unwrap();
    store
        .set_node_property_string(order_only, "title", "NoPath".into())
        .await
        .unwrap();
    store
        .set_node_property_string(order_only, "text", "No path body.".into())
        .await
        .unwrap();
    store
        .create_edge(revision_node, order_only, HAS_PART)
        .await
        .unwrap();

    // Section with neither path_id nor order: typed fallback to source_vid.
    let bare_vid = revision_section_vid(&revision_vid, 9).unwrap();
    let bare = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(bare, "vid", bare_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(bare, "title", "Bare".into())
        .await
        .unwrap();
    store
        .set_node_property_string(bare, "text", "Bare body.".into())
        .await
        .unwrap();
    store
        .create_edge(revision_node, bare, HAS_PART)
        .await
        .unwrap();

    let walk = collect_revision_summary_inputs(store.as_ref() as &dyn DirectGraphStore, 160).await;
    assert_eq!(walk.inputs.len(), 1);
    let input = &walk.inputs[0];
    // 3 seeded + nested + order_only + bare.
    assert_eq!(input.sections.len(), 6);

    // Dotted path wins over order even when the ordinals conflict.
    let nested_input = input
        .sections
        .iter()
        .find(|s| s.source_vid == nested_vid)
        .expect("nested section collected");
    assert_eq!(nested_input.path_id, "3.2");
    // Seeded sections keep their real dotted path, never the missing order.
    let seeded_input = input
        .sections
        .iter()
        .find(|s| s.source_vid == seed_sections[1].source_vid)
        .expect("seeded section collected");
    assert_eq!(seeded_input.path_id, "2");
    // order-only section falls back to the order ordinal, recorded as typed.
    let order_only_input = input
        .sections
        .iter()
        .find(|s| s.source_vid == order_only_vid)
        .expect("order-only section collected");
    assert_eq!(order_only_input.path_id, "9");
    // Bare section falls back to its VID, recorded as typed.
    let bare_input = input
        .sections
        .iter()
        .find(|s| s.source_vid == bare_vid)
        .expect("bare section collected");
    assert_eq!(bare_input.path_id, bare_vid);

    assert_eq!(walk.path_id_fallbacks.len(), 2);
    let order_fallback = walk
        .path_id_fallbacks
        .iter()
        .find(|f| f.section_node == order_only)
        .expect("order fallback diagnostic");
    assert_eq!(order_fallback.reason, "path_id_missing_fallback_to_order");
    assert_eq!(order_fallback.path_id, "9");
    assert_eq!(order_fallback.source_vid, order_only_vid);
    let source_fallback = walk
        .path_id_fallbacks
        .iter()
        .find(|f| f.section_node == bare)
        .expect("source_vid fallback diagnostic");
    assert_eq!(
        source_fallback.reason,
        "path_id_missing_fallback_to_source_vid"
    );
    assert_eq!(source_fallback.path_id, bare_vid);
}

#[tokio::test]
async fn walker_does_not_invent_revision_for_unrelated_labels() {
    use da_application::extractive_summary::collect_revision_summary_inputs;
    let store = Arc::new(MockGraphStore::new());
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "paper_id", "2507.19457".into())
        .await
        .unwrap();
    let walk = collect_revision_summary_inputs(store.as_ref() as &dyn DirectGraphStore, 160).await;
    assert!(walk.inputs.is_empty());
    assert!(walk.skipped.is_empty());
}

#[tokio::test]
async fn summary_rerun_with_changed_text_supersedes_previous_version() {
    // PIR-012: a rerun with a different source text must close the previous
    // current Summary (valid_to, superseded_at, superseded_by,
    // retrieval_eligible=false) and write a new version, leaving both
    // queryable. Identity: same logical_vid, different content-addressed vid.
    let store = Arc::new(MockGraphStore::new());
    let clock = Arc::new(MutableClock::new(ts(1_700_000_000)));
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .with_clock(clock.clone());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts with Pareto selection.").await;

    let mut first_input = input(revision_node, revision_vid.clone(), sections.clone());
    use_case.materialize(first_input.clone()).await.unwrap();

    // Rerun with changed section text at a later instant.
    clock.set(ts(1_700_000_100));
    first_input.sections[1].text = "GEPA now uses reflective evolution.".into();
    use_case.materialize(first_input).await.unwrap();

    let logical_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let group = summary_group_nodes(&store, &logical_vid).await;
    assert_eq!(group.len(), 2, "both versions remain in the graph");

    // The group contains exactly one current node (the new version).
    let current = current_summary_node(&store, &logical_vid).await.unwrap();
    let superseded = group.iter().copied().find(|n| *n != current).unwrap();

    // Old version: closed at t2, points at the new version, off the
    // retrieval surface, but its text/digest history is intact.
    assert_eq!(
        store
            .get_node_property_string(superseded, "valid_to")
            .await
            .as_deref(),
        Some(ts(1_700_000_100).to_rfc3339().as_str())
    );
    assert_eq!(
        store
            .get_node_property_string(superseded, "superseded_at")
            .await
            .as_deref(),
        Some(ts(1_700_000_100).to_rfc3339().as_str())
    );
    let current_vid = store
        .get_node_property_string(current, "vid")
        .await
        .unwrap();
    assert_eq!(
        store
            .get_node_property_string(superseded, "superseded_by")
            .await
            .as_deref(),
        Some(current_vid.as_str())
    );
    assert!(
        !store
            .get_node_property_bool(superseded, "retrieval_eligible")
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_node_property_string(superseded, "text")
            .await
            .as_deref(),
        Some("GEPA mutates prompts with Pareto selection.")
    );
    assert_eq!(
        store
            .get_node_property_string(superseded, "logical_vid")
            .await
            .as_deref(),
        Some(logical_vid.as_str())
    );

    // New version: current, opened at t2, carries the new text.
    assert_eq!(
        store
            .get_node_property_string(current, "valid_from")
            .await
            .as_deref(),
        Some(ts(1_700_000_100).to_rfc3339().as_str())
    );
    assert_eq!(
        store
            .get_node_property_string(current, "recorded_at")
            .await
            .as_deref(),
        Some(ts(1_700_000_100).to_rfc3339().as_str())
    );
    assert!(
        store
            .get_node_property_bool(current, "retrieval_eligible")
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_node_property_string(current, "text")
            .await
            .as_deref(),
        Some("GEPA now uses reflective evolution.")
    );
    assert!(
        !store
            .get_node_property_bool(current, "import_eligible")
            .await
            .unwrap(),
        "D127: import_eligible stays false"
    );
}

#[tokio::test]
async fn summary_rerun_identical_content_is_replay_idempotent() {
    // PIR-012 replay gate: identical content+policy must stay ONE current
    // node — no new version, no closed sibling, no history churn.
    let store = Arc::new(MockGraphStore::new());
    let clock = Arc::new(MutableClock::new(ts(1_700_000_000)));
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .with_clock(clock.clone());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    let materialize_input = input(revision_node, revision_vid.clone(), sections.clone());
    use_case
        .materialize(materialize_input.clone())
        .await
        .unwrap();
    clock.set(ts(1_700_000_100));
    use_case.materialize(materialize_input).await.unwrap();

    let logical_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let group = summary_group_nodes(&store, &logical_vid).await;
    assert_eq!(group.len(), 1, "identical replay must not add a version");
    let node = group[0];
    // Still current: interval opened at t1, no close markers anywhere.
    assert_eq!(
        store
            .get_node_property_string(node, "valid_from")
            .await
            .as_deref(),
        Some(ts(1_700_000_000).to_rfc3339().as_str())
    );
    assert_eq!(store.get_node_property_string(node, "valid_to").await, None);
    assert_eq!(
        store.get_node_property_string(node, "superseded_at").await,
        None
    );
    assert!(
        store
            .get_node_property_bool(node, "retrieval_eligible")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn summary_version_reactivates_when_content_returns() {
    // PIR-012 replay convergence: v1 → v2 (changed) → v1 again must
    // re-activate the v1 node (one current node), not duplicate its vid.
    let store = Arc::new(MockGraphStore::new());
    let clock = Arc::new(MutableClock::new(ts(1_700_000_000)));
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .with_clock(clock.clone());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    let base = input(revision_node, revision_vid.clone(), sections.clone());

    use_case.materialize(base.clone()).await.unwrap();
    clock.set(ts(1_700_000_100));
    let mut changed = base.clone();
    changed.sections[1].text = "Different body text for the rerun.".into();
    use_case.materialize(changed).await.unwrap();
    clock.set(ts(1_700_000_200));
    use_case.materialize(base).await.unwrap();

    let logical_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();
    let group = summary_group_nodes(&store, &logical_vid).await;
    assert_eq!(group.len(), 2, "no third node: the v1 node was reactivated");
    let current = current_summary_node(&store, &logical_vid).await.unwrap();
    // Current node is the reactivated v1: text matches v1, interval reopened.
    assert_eq!(
        store
            .get_node_property_string(current, "text")
            .await
            .as_deref(),
        Some("GEPA mutates prompts.")
    );
    assert_eq!(
        store
            .get_node_property_string(current, "valid_from")
            .await
            .as_deref(),
        Some(ts(1_700_000_200).to_rfc3339().as_str())
    );
    assert!(
        store
            .get_node_property_string(current, "superseded_at")
            .await
            .as_deref()
            .is_none_or(|s| s.is_empty()),
        "reactivated version carries no close marker"
    );
    assert!(
        store
            .get_node_property_bool(current, "retrieval_eligible")
            .await
            .unwrap()
    );
    // The v2 node is now the closed one, superseded by the v1 version vid.
    let closed = group.iter().copied().find(|n| *n != current).unwrap();
    let current_vid = store
        .get_node_property_string(current, "vid")
        .await
        .unwrap();
    assert_eq!(
        store
            .get_node_property_string(closed, "valid_to")
            .await
            .as_deref(),
        Some(ts(1_700_000_200).to_rfc3339().as_str())
    );
    assert_eq!(
        store
            .get_node_property_string(closed, "superseded_by")
            .await
            .as_deref(),
        Some(current_vid.as_str())
    );
    assert!(
        !store
            .get_node_property_bool(closed, "retrieval_eligible")
            .await
            .unwrap()
    );
}

/// Build a universal PropertyMap from a Summary node's temporal fields so the
/// kg-ontology bitemporal helpers can be asserted directly (PIR-012 proof).
async fn temporal_props(store: &MockGraphStore, node: u64) -> kg_ontology::bitemporal::PropertyMap {
    use kg_ontology::bitemporal::PropertyMap;
    let mut props = PropertyMap::new();
    for key in ["valid_from", "valid_to", "recorded_at", "superseded_at"] {
        if let Some(value) = store.get_node_property_string(node, key).await {
            props.insert(key.to_string(), serde_json::Value::String(value));
        }
    }
    props
}

#[tokio::test]
async fn summary_as_of_reads_current_and_historical_versions() {
    // PIR-012 as-of proof: after a rerun with changed text, the old version
    // stays queryable at its own instant and the new version at its own —
    // adjacent non-overlapping intervals (old.valid_to == new.valid_from), all
    // through the same-process get_summary_as_of read surface.
    use da_application::extractive_summary::{get_summary_as_of, list_summary_versions};
    use kg_ontology::bitemporal::{is_active_at, validate_bitemporal, was_known_at};

    let store = Arc::new(MockGraphStore::new());
    let clock = Arc::new(MutableClock::new(ts(1_700_000_000)));
    let use_case = ExtractiveSummaryUseCase::new(store.clone() as Arc<dyn DirectGraphStore>)
        .with_clock(clock.clone());
    let (revision_node, revision_vid, sections) =
        seed_revision(&store, 1, "GEPA mutates prompts.").await;
    let base = input(revision_node, revision_vid.clone(), sections.clone());
    use_case.materialize(base.clone()).await.unwrap();

    clock.set(ts(1_700_000_100));
    let mut changed = base.clone();
    changed.sections[1].text = "GEPA now uses reflective evolution.".into();
    use_case.materialize(changed).await.unwrap();

    let store_ref = store.as_ref() as &dyn DirectGraphStore;
    let logical_vid =
        revision_summary_vid(&revision_vid, "section", &sections[1].source_vid).unwrap();

    // Before the first write nothing was valid/known yet.
    assert!(
        get_summary_as_of(store_ref, &logical_vid, ts(1_699_999_999))
            .await
            .is_none()
    );

    // At t1 the v1 version (old text) is active AND known.
    let at_t1 = get_summary_as_of(store_ref, &logical_vid, ts(1_700_000_000))
        .await
        .expect("v1 readable at its own instant");
    assert_eq!(at_t1.text, "GEPA mutates prompts.");
    assert_eq!(at_t1.valid_from, Some(ts(1_700_000_000)));
    assert_eq!(at_t1.valid_to, Some(ts(1_700_000_100)));
    assert_eq!(at_t1.superseded_at, Some(ts(1_700_000_100)));
    assert!(!at_t1.retrieval_eligible);
    // PIR-007: the dotted AST path survives onto the version snapshot.
    assert_eq!(at_t1.source_path_id, sections[1].path_id);
    assert!(at_t1.source_path_missingness.is_empty());
    assert!(at_t1.bitemporal_issues.is_empty());

    // Boundary instant: intervals are [from, to) — exactly at t2 the NEW
    // version becomes active/known.
    let at_t2 = get_summary_as_of(store_ref, &logical_vid, ts(1_700_000_100))
        .await
        .expect("v2 readable at its own instant");
    assert_eq!(at_t2.text, "GEPA now uses reflective evolution.");
    assert_eq!(at_t2.valid_from, Some(ts(1_700_000_100)));
    assert_eq!(at_t2.valid_to, None);
    assert_eq!(at_t2.superseded_at, None);
    assert!(at_t2.retrieval_eligible);
    assert!(at_t2.bitemporal_issues.is_empty());

    // Later: the current version is still v2.
    let later = get_summary_as_of(store_ref, &logical_vid, ts(1_700_000_500))
        .await
        .expect("current readable");
    assert_eq!(later.vid, at_t2.vid);

    // Adjacent, non-overlapping valid intervals.
    assert_eq!(at_t1.valid_to, at_t2.valid_from);

    // Full history is queryable: both versions, oldest first, texts intact.
    let history = list_summary_versions(store_ref, &logical_vid).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].text, "GEPA mutates prompts.");
    assert_eq!(history[1].text, "GEPA now uses reflective evolution.");
    assert_eq!(history[0].valid_to, history[1].valid_from);
    assert_eq!(history[1].valid_to, None);

    // Direct kg-ontology bitemporal proof on the raw node snapshots: v1 is
    // active+known at t1 and neither at t2; v2 is active+known from t2 on.
    let group = summary_group_nodes(&store, &logical_vid).await;
    assert_eq!(group.len(), 2);
    let mut v1_node = None;
    let mut v2_node = None;
    for n in group {
        let text = store.get_node_property_string(n, "text").await;
        if text.as_deref() == Some("GEPA mutates prompts.") {
            v1_node = Some(n);
        } else if text.as_deref() == Some("GEPA now uses reflective evolution.") {
            v2_node = Some(n);
        }
    }
    let v1_node = v1_node.expect("v1 node in group");
    let v2_node = v2_node.expect("v2 node in group");
    let v1_props = temporal_props(&store, v1_node).await;
    let v2_props = temporal_props(&store, v2_node).await;
    assert!(is_active_at(&v1_props, ts(1_700_000_000)));
    assert!(was_known_at(&v1_props, ts(1_700_000_000)));
    assert!(!is_active_at(&v1_props, ts(1_700_000_100)));
    assert!(!was_known_at(&v1_props, ts(1_700_000_100)));
    assert!(is_active_at(&v2_props, ts(1_700_000_100)));
    assert!(was_known_at(&v2_props, ts(1_700_000_100)));
    assert!(is_active_at(&v2_props, ts(1_700_000_500)));
    assert!(was_known_at(&v2_props, ts(1_700_000_500)));
    assert!(validate_bitemporal(&v1_props).is_empty());
    assert!(validate_bitemporal(&v2_props).is_empty());
}

#[tokio::test]
async fn summary_as_of_unknown_group_and_legacy_node() {
    // Negative: an unknown logical group has no versions at any instant.
    // Legacy: a pre-T03 Summary node (vid == logical_vid, no temporal fields)
    // is active+known at every instant through the same read surface.
    use da_application::extractive_summary::{get_summary_as_of, list_summary_versions};

    let store = Arc::new(MockGraphStore::new());
    let store_ref = store.as_ref() as &dyn DirectGraphStore;
    let unknown = revision_summary_vid(&"a".repeat(64), "section", &"b".repeat(64)).unwrap();
    assert!(
        get_summary_as_of(store_ref, &unknown, ts(1_700_000_000))
            .await
            .is_none()
    );
    assert!(list_summary_versions(store_ref, &unknown).await.is_empty());

    // Legacy node written by the pre-T03 writer shape: vid == logical_vid, no
    // logical_vid property, no temporal fields. Missing bounds mean OPEN on
    // both axes, so the node is readable at any instant.
    let legacy_vid = revision_summary_vid(&"c".repeat(64), "abstract", &"c".repeat(64)).unwrap();
    let node = store.create_node("Summary").await.unwrap();
    store
        .set_node_property_string(node, "vid", legacy_vid.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(node, "text", "legacy digest".into())
        .await
        .unwrap();
    for at in [ts(1_000_000_000), ts(1_700_000_000), ts(2_000_000_000)] {
        let read = get_summary_as_of(store_ref, &legacy_vid, at)
            .await
            .unwrap_or_else(|| panic!("legacy summary readable at {at}"));
        assert_eq!(read.text, "legacy digest");
        assert_eq!(read.vid, legacy_vid);
        assert!(read.logical_vid.is_empty());
        assert_eq!(read.valid_from, None);
        assert_eq!(read.valid_to, None);
    }
    let history = list_summary_versions(store_ref, &legacy_vid).await;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, "legacy digest");
}

#[tokio::test]
async fn legacy_integer_wire_summary_decodes_through_bounded_decoder() {
    // PIR-008 (M398/S04/T03): a pre-T03 Summary node with INTEGER (legacy)
    // temporal properties — valid_from/recorded_at/superseded_at as int props —
    // must decode through the bounded compatibility decoder. As-of selection
    // returns it inside [valid_from, superseded_at), supersession hides it
    // from the current surface, and history ordering compares parsed
    // instants — never lexical integer/string byte order.
    use da_application::extractive_summary::{get_summary_as_of, list_summary_versions};

    let store = Arc::new(MockGraphStore::new());
    let store_ref = store.as_ref() as &dyn DirectGraphStore;
    let logical = revision_summary_vid(&"e".repeat(64), "abstract", &"e".repeat(64)).unwrap();

    // v1: legacy integer wire, superseded at 1010000000 (2002-01-02T14:13:20Z).
    // Its valid_from 999999999 (2001-09-09T01:46:39Z) is LEXICALLY GREATER than
    // "2002-…" ("9" > "2"), so a byte-order sort would misorder it — the
    // history must order by parsed instant instead.
    let v1 = store.create_node("Summary").await.unwrap();
    store
        .set_node_property_string(v1, "vid", format!("{logical}#v1"))
        .await
        .unwrap();
    store
        .set_node_property_string(v1, "logical_vid", logical.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(v1, "text", "legacy integer v1".into())
        .await
        .unwrap();
    store
        .set_node_property_int(v1, "valid_from", 999_999_999)
        .await
        .unwrap();
    store
        .set_node_property_int(v1, "recorded_at", 999_999_999)
        .await
        .unwrap();
    store
        .set_node_property_int(v1, "superseded_at", 1_010_000_000)
        .await
        .unwrap();
    store
        .set_node_property_bool(v1, "retrieval_eligible", false)
        .await
        .unwrap();

    // v2: canonical RFC3339 UTC string wire, still current (no superseded_at),
    // valid_from "2002-01-03T00:00:00Z" (unix 1010006720).
    let v2 = store.create_node("Summary").await.unwrap();
    store
        .set_node_property_string(v2, "vid", format!("{logical}#v2"))
        .await
        .unwrap();
    store
        .set_node_property_string(v2, "logical_vid", logical.clone())
        .await
        .unwrap();
    store
        .set_node_property_string(v2, "text", "canonical string v2".into())
        .await
        .unwrap();
    store
        .set_node_property_string(v2, "valid_from", "2002-01-03T00:00:00Z".to_string())
        .await
        .unwrap();
    store
        .set_node_property_string(v2, "recorded_at", "2002-01-03T00:00:00Z".to_string())
        .await
        .unwrap();
    store
        .set_node_property_bool(v2, "retrieval_eligible", true)
        .await
        .unwrap();

    // As-of: v1 valid+known inside [999999999, 1010000000); v2 from 2002-01-03.
    let at_v1 = get_summary_as_of(store_ref, &logical, ts(1_000_000_000))
        .await
        .expect("v1 readable as-of");
    assert_eq!(at_v1.text, "legacy integer v1");
    assert_eq!(at_v1.valid_from, Some(ts(999_999_999)));
    assert_eq!(at_v1.superseded_at, Some(ts(1_010_000_000)));
    let at_v2 = get_summary_as_of(store_ref, &logical, ts(1_010_100_000))
        .await
        .expect("v2 readable as-of");
    assert_eq!(at_v2.text, "canonical string v2");
    assert_eq!(at_v2.valid_from, Some(ts(1_010_016_000)));
    assert_eq!(at_v2.superseded_at, None);

    // History orders by parsed instant: v1 (2001) before v2 (2002). A lexical
    // sort of the raw forms ("999999999" vs "2002-01-03T00:00:00Z") would put
    // v2 first ('2' < '9') — the decoder-backed ordering must not.
    let history = list_summary_versions(store_ref, &logical).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].text, "legacy integer v1");
    assert_eq!(history[1].text, "canonical string v2");
}
