//! Contract test: the unified prompt registry resolves BOTH inline-template
//! prompt tasks and file-based meta-prompt agents on the real tree data, and a
//! call record whose lineage disagrees with it is refused fail-closed.
//!
//! This exercises the real `data/ontology/prompt_tasks.yaml` +
//! `data/ontology/prompt_registry.yaml` + `data/agents/*/manifest.yaml` +
//! `data/prompts/*/system.v1.md` files, so a DATA or prompt edit that breaks the
//! `(prompt_id, prompt_version)` -> schema/fingerprint binding fails the build.

use da_adapters::prompt_registry::load_prompt_registry;
use da_adapters::prompt_task_registry::load_prompt_task_registry;
use da_domain::assurance::ContentDigest;
use da_domain::prompt_optimization::PromptExecutionLineage;
use da_domain::prompt_registry::{
    PromptEntryKind, PromptLineageBindingError, PromptRegistry, PromptRegistryEntry,
    bind_lineage_to_registry,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn real_registry() -> PromptRegistry {
    let root = repo_root();
    let tasks = load_prompt_task_registry(&root.join("data/ontology/prompt_tasks.yaml")).unwrap();
    load_prompt_registry(
        &root.join("data/ontology/prompt_registry.yaml"),
        &tasks,
        &root,
    )
    .unwrap()
}

/// A call record whose `(prompt_id, prompt_version, prompt_sha256)` bind the
/// resolved entry, with an overridable version/fingerprint for the refusal cases.
fn bound_lineage(
    entry: &PromptRegistryEntry,
    prompt_version: &str,
    prompt_sha256: &ContentDigest,
) -> PromptExecutionLineage {
    PromptExecutionLineage {
        run_id: "run-contract".into(),
        prompt_id: entry.prompt_id.clone(),
        prompt_version: prompt_version.into(),
        prompt_sha256: prompt_sha256.clone(),
        task_spec_sha256: ContentDigest::of_text("task-spec"),
        rendered_prompt_sha256: ContentDigest::of_text("rendered"),
        output_schema_sha256: entry.schema_sha256.clone(),
        route_policy_sha256: ContentDigest::of_text("route"),
        endpoint_sha256: ContentDigest::of_text("endpoint"),
        model_binding_sha256: ContentDigest::of_text("model"),
        inference_config_sha256: ContentDigest::of_text("inference"),
        context_bundle_sha256: ContentDigest::of_text("context"),
        provider: "contract".into(),
        model: "model-v1".into(),
        input_sha256: ContentDigest::of_text("input"),
        output_sha256: ContentDigest::of_text("output"),
        input_tokens: 1,
        output_tokens: 1,
        latency_ms: 1,
        temperature_milli: 0,
        max_output_tokens: 16,
        estimated_cost_microunits: None,
        authoritative: false,
        import_eligible: false,
    }
}

fn is_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[test]
fn real_registry_resolves_all_four_prompt_forms() {
    let registry = real_registry();
    assert_eq!(registry.entries.len(), 4);
    assert!(!registry.d127_import_eligible);
    for (id, version) in [
        ("article_summary", "summary.v1"),
        ("figure_analysis", "figure-analysis.v3"),
        ("critic", "1.0.0"),
        ("summarizer", "1.0.0"),
    ] {
        let entry = registry
            .resolve(id, version)
            .unwrap_or_else(|error| panic!("{id}@{version} must resolve: {error}"));
        assert!(is_hex64(entry.prompt_sha256.as_str()));
        assert!(is_hex64(entry.schema_sha256.as_str()));
    }
}

#[test]
fn task_entries_carry_derived_provenance_and_output_schema() {
    let registry = real_registry();
    let summary = registry.resolve("article_summary", "summary.v1").unwrap();
    assert_eq!(summary.kind, PromptEntryKind::Task);
    assert_eq!(
        summary.source_ref,
        "data/ontology/prompt_tasks.yaml#article_summary"
    );
    assert_eq!(summary.schema_ref, "article-summary.v1");
}

#[test]
fn meta_prompt_entries_bind_source_bytes_and_agent_version() {
    let registry = real_registry();
    let critic = registry.resolve("critic", "1.0.0").unwrap();
    let summarizer = registry.resolve("summarizer", "1.0.0").unwrap();
    assert_eq!(critic.kind, PromptEntryKind::MetaPrompt);
    assert_eq!(summarizer.kind, PromptEntryKind::MetaPrompt);
    assert_eq!(critic.source_ref, "data/prompts/critic/system.v1.md");
    assert_eq!(
        critic.schema_ref,
        "data/schemas/agent.schema.yaml".to_string()
    );
    // Fingerprints are computed from the real files, so distinct prompts differ.
    assert_ne!(critic.prompt_sha256, summarizer.prompt_sha256);
    // Both meta prompts share the agent schema, so their schema digest matches.
    assert_eq!(critic.schema_sha256, summarizer.schema_sha256);
}

#[test]
fn unknown_id_and_wrong_version_are_refused() {
    let registry = real_registry();
    assert!(registry.resolve("no_such_prompt", "summary.v1").is_err());
    assert!(registry.resolve("article_summary", "summary.v99").is_err());
    // A version that resolves for a different prompt is still refused.
    assert!(registry.resolve("critic", "summary.v1").is_err());
}

#[test]
fn lineage_binds_to_registry_and_a_tampered_fingerprint_is_refused() {
    let registry = real_registry();
    let entry = registry.resolve("article_summary", "summary.v1").unwrap();

    let good = bound_lineage(entry, &entry.prompt_version, &entry.prompt_sha256);
    assert!(bind_lineage_to_registry(&good, &registry).is_ok());

    let tampered = bound_lineage(
        entry,
        &entry.prompt_version,
        &ContentDigest::of_text("tampered-template"),
    );
    let error = bind_lineage_to_registry(&tampered, &registry).unwrap_err();
    assert!(
        matches!(error, PromptLineageBindingError::FingerprintMismatch { .. }),
        "{error}"
    );
    // A refusal is self-describing: it names the prompt.
    assert!(error.to_string().contains("article_summary"));
}

#[test]
fn lineage_with_wrong_version_or_unknown_id_is_refused() {
    let registry = real_registry();
    let entry = registry.resolve("article_summary", "summary.v1").unwrap();

    let drifted = bound_lineage(entry, "summary.v99", &entry.prompt_sha256);
    assert!(matches!(
        bind_lineage_to_registry(&drifted, &registry).unwrap_err(),
        PromptLineageBindingError::VersionMismatch { .. }
    ));

    let mut unknown = bound_lineage(entry, "summary.v1", &entry.prompt_sha256);
    unknown.prompt_id = "no_such_prompt".into();
    assert!(matches!(
        bind_lineage_to_registry(&unknown, &registry).unwrap_err(),
        PromptLineageBindingError::Unresolved { .. }
    ));
}

#[test]
fn task_lineage_with_a_wrong_output_schema_is_refused() {
    let registry = real_registry();
    let entry = registry
        .resolve("figure_analysis", "figure-analysis.v3")
        .unwrap();
    let mut drifted = bound_lineage(entry, &entry.prompt_version, &entry.prompt_sha256);
    drifted.output_schema_sha256 = ContentDigest::of_text("wrong-schema");
    assert!(matches!(
        bind_lineage_to_registry(&drifted, &registry).unwrap_err(),
        PromptLineageBindingError::SchemaMismatch { .. }
    ));
}
