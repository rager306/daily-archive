//! ADR-059 generative summary candidate authority and lineage contracts.

use async_trait::async_trait;
use da_application::SummaryUseCase;
use da_domain::{
    assurance::ContentDigest,
    prompt_optimization::{
        PromptExecutionLineage, PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec,
        ResolvedModelBinding,
    },
    prompt_registry::{
        PROMPT_REGISTRY_SCHEMA_VERSION, PromptEntryKind, PromptRegistry, PromptRegistryEntry,
    },
    vid,
};
use da_ports::{
    FixedClock,
    graph_store::DirectGraphStore,
    parser::ParsedArticle,
    prompt_execution::{
        PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptExecutionResponse,
    },
};
use std::sync::Arc;
mod common;
use common::mock_graph_store::MockGraphStore;

struct FixedPrompt {
    section_path: &'static str,
    fail: bool,
    /// Return a lineage whose template fingerprint disagrees with the registry.
    tamper_prompt_sha256: bool,
}
#[async_trait]
impl PromptExecutionPort for FixedPrompt {
    async fn execute(
        &self,
        request: PromptExecutionRequest,
    ) -> Result<PromptExecutionResponse, PromptExecutionError> {
        if self.fail {
            return Err(PromptExecutionError::Provider("bounded failure".into()));
        }
        let output=serde_json::to_vec(&serde_json::json!({"abstract_summary":"Grounded summary.","section_summaries":[{"path_id":self.section_path,"title":"Introduction","summary":"Grounded section candidate."}],"methods":[{"name":"GEPA","description":"candidate","how_used":"candidate"}],"datasets":[{"name":"ARC","description":"candidate"}],"metrics":[{"name":"Accuracy","value":"89%","baseline":"83%"}],"quantitative_results":"89%","limitations":"fixture","reproducibility":{"score":4,"justification":"fixture"},"tags":["candidate"]})).unwrap();
        let prompt_sha256 = if self.tamper_prompt_sha256 {
            ContentDigest::of_text("tampered-template")
        } else {
            request.task.template_sha256().unwrap()
        };
        let lineage = PromptExecutionLineage {
            run_id: request.run_id,
            prompt_id: request.task.task_id.clone(),
            prompt_version: request.task.version.clone(),
            prompt_sha256,
            task_spec_sha256: request.task.spec_sha256().unwrap(),
            rendered_prompt_sha256: ContentDigest::of_text("rendered"),
            output_schema_sha256: request.output_schema.sha256().unwrap(),
            route_policy_sha256: ContentDigest::of_text("route-bytes"),
            endpoint_sha256: ContentDigest::of_text("endpoint"),
            model_binding_sha256: request.resolved_model.sha256().unwrap(),
            inference_config_sha256: ContentDigest::of_text("inference"),
            context_bundle_sha256: ContentDigest::of_bytes(&request.input_bytes),
            provider: "fixed".into(),
            model: "fixed-model".into(),
            input_sha256: ContentDigest::of_bytes(&request.input_bytes),
            output_sha256: ContentDigest::of_bytes(&output),
            input_tokens: 10,
            output_tokens: 20,
            latency_ms: 5,
            temperature_milli: request.task.temperature_milli,
            max_output_tokens: request.task.max_output_tokens,
            estimated_cost_microunits: None,
            authoritative: false,
            import_eligible: false,
        };
        Ok(PromptExecutionResponse {
            output_bytes: output,
            lineage,
        })
    }
}
fn contracts() -> (
    PromptTaskSpec,
    PromptRoutePolicy,
    ResolvedModelBinding,
    PromptOutputSchema,
) {
    (
        PromptTaskSpec {
            task_id: "article_summary".into(),
            version: "v1".into(),
            system_template: "System".into(),
            user_template: "{paper_id}{title}{abstract}{full_text}".into(),
            output_schema_id: "summary".into(),
            route_policy_id: "route".into(),
            temperature_milli: 300,
            max_output_tokens: 1000,
            eval_suite_id: "eval".into(),
            optimizer_enabled: false,
        },
        PromptRoutePolicy {
            route_policy_id: "route".into(),
            model_binding_id: "binding".into(),
            endpoint_override_env: None,
            credential_env: "TEST_KEY".into(),
            supports_images: false,
            maximum_temperature_milli: 1000,
        },
        ResolvedModelBinding {
            binding_id: "binding".into(),
            model_id: "fixed".into(),
            provider: "fixed".into(),
            protocol: "test".into(),
            endpoint: "https://example.invalid".into(),
            model_name: "fixed-model".into(),
            tool_version: "1".into(),
            policy_version: "1".into(),
        },
        PromptOutputSchema {
            output_schema_id: "summary".into(),
            schema: serde_json::json!({"type":"object"}),
        },
    )
}
fn prompt_registry(task: &PromptTaskSpec, schema: &PromptOutputSchema) -> PromptRegistry {
    PromptRegistry {
        schema_version: PROMPT_REGISTRY_SCHEMA_VERSION.into(),
        registry_id: "test-prompt-registry".into(),
        d127_import_eligible: false,
        entries: vec![PromptRegistryEntry {
            prompt_id: task.task_id.clone(),
            prompt_version: task.version.clone(),
            kind: PromptEntryKind::Task,
            // Computed from the same test contracts, never hand-picked, so the
            // happy-path lineage binds and a tampered record is refused.
            prompt_sha256: task.template_sha256().unwrap(),
            schema_ref: schema.output_schema_id.clone(),
            schema_sha256: schema.sha256().unwrap(),
            source_ref: "data/ontology/prompt_tasks.yaml#article_summary".into(),
        }],
    }
}
async fn setup() -> (MockGraphStore, u64) {
    let store = MockGraphStore::new();
    let paper = store.create_node("Paper").await.unwrap();
    store
        .set_node_property_string(paper, "vid", vid::paper_vid("2507.19457"))
        .await
        .unwrap();
    let section = store.create_node("Section").await.unwrap();
    store
        .set_node_property_string(section, "vid", "vid:section:2507.19457:1".into())
        .await
        .unwrap();
    (store, paper)
}
fn parsed() -> ParsedArticle {
    ParsedArticle {
        paper_id: "2507.19457".into(),
        title: "GEPA".into(),
        abstract_text: "Abstract".into(),
        body_text: "Body".into(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
        pdf_hash: "a".repeat(64),
        ..Default::default()
    }
}
fn use_case(store: &MockGraphStore, prompt: FixedPrompt) -> SummaryUseCase {
    let (t, r, m, s) = contracts();
    let registry = prompt_registry(&t, &s);
    SummaryUseCase::new(
        Arc::new(store.clone()),
        Arc::new(prompt),
        t,
        r,
        m,
        s,
        registry,
    )
    .unwrap()
    .with_clock(Arc::new(FixedClock::from_timestamp(1_700_000_000)))
}

#[tokio::test]
async fn lineage_bound_candidates_are_nonretrieval_and_never_create_semantic_entities() {
    let (store, paper) = setup().await;
    let result = use_case(
        &store,
        FixedPrompt {
            section_path: "1",
            fail: false,
            tamper_prompt_sha256: false,
        },
    )
    .generate_summaries("2507.19457", paper, &parsed())
    .await
    .unwrap();
    assert_eq!(result.methods_count, 0);
    assert_eq!(result.datasets_count, 0);
    assert_eq!(result.metrics_count, 0);
    assert!(result.prompt_lineage.is_some());
    for label in ["Method", "Dataset", "Metric", "QualitySignal"] {
        assert!(
            store.get_nodes_by_label(label).await.is_empty(),
            "LLM must not create {label}"
        );
    }
    let summaries = store.get_nodes_by_label("Summary").await;
    assert_eq!(summaries.len(), 2);
    for node in summaries {
        let snap = store.snapshot_node(node).unwrap();
        assert_eq!(
            snap.get("retrieval_eligible").and_then(|x| x.as_bool()),
            Some(false)
        );
        assert_eq!(
            snap.get("authoritative").and_then(|x| x.as_bool()),
            Some(false)
        );
        assert_eq!(
            snap.get("import_eligible").and_then(|x| x.as_bool()),
            Some(false)
        );
        assert_eq!(
            snap.get("prompt_task_id").and_then(|x| x.as_str()),
            Some("article_summary")
        );
        assert!(snap.contains_key("output_schema_sha256"));
        assert!(snap.contains_key("model_binding_sha256"));
    }
}
#[tokio::test]
async fn unresolved_section_fails_before_any_graph_mutation() {
    let (store, paper) = setup().await;
    let before = store.get_nodes_by_label("Summary").await.len();
    let error = use_case(
        &store,
        FixedPrompt {
            section_path: "missing",
            fail: false,
            tamper_prompt_sha256: false,
        },
    )
    .generate_summaries("2507.19457", paper, &parsed())
    .await
    .unwrap_err();
    assert!(error.to_string().contains("does not resolve"));
    assert_eq!(store.get_nodes_by_label("Summary").await.len(), before);
}
#[tokio::test]
async fn provider_failure_is_visible_and_writes_nothing() {
    let (store, paper) = setup().await;
    let error = use_case(
        &store,
        FixedPrompt {
            section_path: "1",
            fail: true,
            tamper_prompt_sha256: false,
        },
    )
    .generate_summaries("2507.19457", paper, &parsed())
    .await
    .unwrap_err();
    assert!(error.to_string().contains("bounded failure"));
    assert!(store.get_nodes_by_label("Summary").await.is_empty());
}
#[tokio::test]
async fn tampered_lineage_is_refused_at_the_write_seam() {
    let (store, paper) = setup().await;
    let error = use_case(
        &store,
        FixedPrompt {
            section_path: "1",
            fail: false,
            tamper_prompt_sha256: true,
        },
    )
    .generate_summaries("2507.19457", paper, &parsed())
    .await
    .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("summary execution lineage refused"),
        "refusal must name the summary write seam: {message}"
    );
    assert!(
        message.contains("fingerprint mismatch") && message.contains("article_summary"),
        "refusal must carry the typed reason and prompt id: {message}"
    );
    assert!(
        store.get_nodes_by_label("Summary").await.is_empty(),
        "a lineage the registry refuses must not write a Summary candidate"
    );
}
#[test]
fn construction_refuses_a_registry_that_disagrees_with_the_contract() {
    let store = MockGraphStore::new();
    let (t, r, m, s) = contracts();

    // The registry declares the task at a version the local contract does not use.
    let mut drifted_version = prompt_registry(&t, &s);
    drifted_version.entries[0].prompt_version = "v2".into();
    let error = SummaryUseCase::new(
        Arc::new(store.clone()),
        Arc::new(FixedPrompt {
            section_path: "1",
            fail: false,
            tamper_prompt_sha256: false,
        }),
        t.clone(),
        r.clone(),
        m.clone(),
        s.clone(),
        drifted_version,
    )
    .err()
    .expect("a registry with an unregistered version must fail construction");
    assert!(
        error.to_string().contains("does not resolve"),
        "an unregistered version must be refused at construction: {error}"
    );

    // The registry resolves the version but records a fingerprint the contract disagrees with.
    let mut drifted_fingerprint = prompt_registry(&t, &s);
    drifted_fingerprint.entries[0].prompt_sha256 = ContentDigest::of_text("tampered-template");
    let error = SummaryUseCase::new(
        Arc::new(store.clone()),
        Arc::new(FixedPrompt {
            section_path: "1",
            fail: false,
            tamper_prompt_sha256: false,
        }),
        t,
        r,
        m,
        s,
        drifted_fingerprint,
    )
    .err()
    .expect("a registry with a drifted fingerprint must fail construction");
    assert!(
        error
            .to_string()
            .contains("disagrees with the local task/schema contract"),
        "a drifted fingerprint must be refused at construction: {error}"
    );
}
