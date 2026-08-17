//! Bounded real-Samyama canary for ontology v2 endpoint materializers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use da_adapters::SamyamaGraphStore;
use da_application::ontology_contract_audit::observe_runtime_graph;
use da_application::ontology_endpoint_evidence::{
    ENDPOINT_EVIDENCE_SCHEMA_VERSION, OntologyEndpointEvidence,
};
use da_application::{
    EnrichUseCase, GraphFinalizeUseCase, PipelineRunReport, StageDisposition, StageRunRecord,
    SummaryUseCase,
};
use da_cli::gold_graph::atomic_write;
use da_cli::graph_tasks::{parse_tasks_cypher, run_graph_task_suite};
use da_domain::{
    ContentDigest,
    finalize::{FinalizePolicy, FinalizeVerdict},
    prompt_optimization::{
        PromptExecutionLineage, PromptOutputSchema, PromptRoutePolicy, PromptTaskSpec,
        ResolvedModelBinding,
    },
    vid,
};
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::{OpenAlexClient, OpenAlexError, OpenAlexResult, OpenAlexWork};
use da_ports::parser::ParsedArticle;
use da_ports::prompt_execution::{
    PromptExecutionError, PromptExecutionPort, PromptExecutionRequest, PromptExecutionResponse,
};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long,
        default_value = "artifacts/ontology/ontology_v2_endpoint_canary.json"
    )]
    report: PathBuf,
}

struct FixedOpenAlex;

#[async_trait]
impl OpenAlexClient for FixedOpenAlex {
    async fn fetch_by_arxiv_id(&self, _arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        Ok(OpenAlexWork {
            id: "https://openalex.org/W-CANARY".into(),
            title: "Ontology endpoint canary".into(),
            doi: None,
            publication_date: Some("2026-01-01".into()),
            cited_by_count: 0,
            primary_topic: None,
            topics: vec![],
            concepts: vec![],
            authors: vec![],
            institutions: vec![],
            authorships: vec![],
            referenced_works: vec![
                "https://openalex.org/W1001".into(),
                "https://openalex.org/W1002".into(),
            ],
        })
    }

    async fn search(&self, _query: &str, _limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        Err(OpenAlexError::NotFound("canary search disabled".into()))
    }
}

struct FixedSummary;

#[async_trait]
impl PromptExecutionPort for FixedSummary {
    async fn execute(
        &self,
        request: PromptExecutionRequest,
    ) -> Result<PromptExecutionResponse, PromptExecutionError> {
        let output = serde_json::to_vec(&serde_json::json!({
            "abstract_summary": "A bounded ontology endpoint canary.",
            "section_summaries": [{"path_id":"1","title":"Introduction","summary":"Introduces the canary."}],
            "methods": [], "datasets": [], "metrics": [],
            "quantitative_results": "Two citation stubs are preserved.",
            "limitations": "The canary is intentionally bounded and synthetic.",
            "reproducibility": {"score": 5, "justification": "Deterministic fixtures and real Samyama."},
            "tags": ["ontology-v2"]
        })).unwrap();
        Ok(PromptExecutionResponse {
            lineage: PromptExecutionLineage {
                run_id: request.run_id,
                task_id: request.task.task_id.clone(),
                task_spec_sha256: request.task.spec_sha256().unwrap(),
                rendered_prompt_sha256: ContentDigest::of_text("canary-prompt"),
                output_schema_sha256: request.output_schema.sha256().unwrap(),
                route_policy_sha256: ContentDigest::of_text("canary-route"),
                endpoint_sha256: ContentDigest::of_text("canary-endpoint"),
                model_binding_sha256: request.resolved_model.sha256().unwrap(),
                provider: "fixed-ontology-canary".into(),
                model: "fixed".into(),
                input_sha256: ContentDigest::of_bytes(&request.input_bytes),
                output_sha256: ContentDigest::of_bytes(&output),
                input_tokens: 1,
                output_tokens: 1,
                latency_ms: 0,
                temperature_milli: request.task.temperature_milli,
                max_output_tokens: request.task.max_output_tokens,
                estimated_cost_microunits: None,
                authoritative: false,
                import_eligible: false,
            },
            output_bytes: output,
        })
    }
}

fn summary_contracts() -> (
    PromptTaskSpec,
    PromptRoutePolicy,
    ResolvedModelBinding,
    PromptOutputSchema,
) {
    (
        PromptTaskSpec {
            task_id: "article_summary".into(),
            version: "canary.v1".into(),
            system_template: "system".into(),
            user_template: "{paper_id}{title}{abstract}{full_text}".into(),
            output_schema_id: "summary".into(),
            route_policy_id: "fixed".into(),
            temperature_milli: 0,
            max_output_tokens: 1000,
            eval_suite_id: "canary".into(),
            optimizer_enabled: false,
        },
        PromptRoutePolicy {
            route_policy_id: "fixed".into(),
            model_binding_id: "fixed-binding".into(),
            endpoint_override_env: None,
            credential_env: "CANARY_KEY".into(),
            supports_images: false,
            maximum_temperature_milli: 0,
        },
        ResolvedModelBinding {
            binding_id: "fixed-binding".into(),
            model_id: "fixed".into(),
            provider: "fixed-ontology-canary".into(),
            protocol: "fixed".into(),
            endpoint: "https://example.invalid".into(),
            model_name: "fixed".into(),
            tool_version: "1".into(),
            policy_version: "1".into(),
        },
        PromptOutputSchema {
            output_schema_id: "summary".into(),
            schema: serde_json::json!({"type":"object"}),
        },
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("ontology_endpoint_canary");
    let args = Args::parse();
    ensure_report_path(&args.report)?;
    let store = Arc::new(SamyamaGraphStore::try_from_env().map_err(anyhow::Error::msg)?);
    let direct_store: Arc<dyn DirectGraphStore> = store.clone();
    let paper_id = "9999.00001";
    let paper = store.create_node("Paper").await?;
    for (key, value) in [
        ("vid", vid::paper_vid(paper_id)),
        ("arxiv_id", paper_id.to_string()),
        ("title", "Ontology endpoint canary".into()),
    ] {
        store.set_node_property_string(paper, key, value).await?;
    }
    store
        .set_node_property_bool(paper, "import_eligible", false)
        .await?;
    store
        .set_node_property_bool(paper, "retrieval_eligible", true)
        .await?;
    // PIR-008 / M398-S04: node DateTime properties use the canonical RFC3339 UTC
    // string wire form; fixed instant keeps the canary graph replay-deterministic
    // (matches the fixed created_at/valid_at edge instants below).
    store
        .set_node_property_string(paper, "valid_from", "2023-11-14T22:13:20Z".to_string())
        .await?;
    store
        .set_node_property_int(paper, "schema_version", 1)
        .await?;

    let section = store.create_node("Section").await?;
    for (key, value) in [
        ("vid", format!("vid:section:{paper_id}:1")),
        ("title", "Introduction".into()),
        ("work_vid", vid::paper_vid(paper_id)),
        ("path_id", "1".into()),
    ] {
        store.set_node_property_string(section, key, value).await?;
    }
    store.set_node_property_int(section, "level", 1).await?;
    store.set_node_property_int(section, "order", 0).await?;
    store
        .set_node_property_bool(section, "import_eligible", false)
        .await?;
    store
        .set_node_property_bool(section, "retrieval_eligible", true)
        .await?;
    store
        .set_node_property_int(section, "schema_version", 1)
        .await?;

    let enrich = EnrichUseCase::from_shared(Arc::new(FixedOpenAlex), direct_store.clone());
    let first = enrich.enrich_by_arxiv_id(paper_id).await?;
    let (summary_task, summary_route, summary_model, summary_schema) = summary_contracts();
    let summary = SummaryUseCase::new(
        direct_store.clone(),
        Arc::new(FixedSummary),
        summary_task,
        summary_route,
        summary_model,
        summary_schema,
    )?;
    summary
        .generate_summaries(paper_id, paper, &parsed(paper_id))
        .await?;
    materialize_quality_signal_fixtures(direct_store.as_ref(), paper, paper_id).await?;

    let before = observe_runtime_graph(store.as_ref(), "process-local-samyama-canary").await;
    let before_target_edges = target_endpoint_projection(&before);
    let replay = enrich.enrich_by_arxiv_id(paper_id).await?;
    summary
        .generate_summaries(paper_id, paper, &parsed(paper_id))
        .await?;
    let after = observe_runtime_graph(store.as_ref(), "process-local-samyama-canary").await;

    let citation_nodes = store.get_nodes_by_label("Citation").await;
    let quality_nodes = store.get_nodes_by_label("QualitySignal").await;
    let paper_nodes = store.get_nodes_by_label("Paper").await;
    let cites_edges = count_edges(store.as_ref(), paper, &citation_nodes, "CITES").await;
    let quality_signal_edges =
        count_edges(store.as_ref(), paper, &quality_nodes, "hasQualitySignal").await;
    let forbidden_paper_to_paper_cites =
        count_edges(store.as_ref(), paper, &paper_nodes, "CITES").await;
    let forbidden_summary_to_quality_signal =
        count_edges(store.as_ref(), paper, &quality_nodes, "hasSummary").await;
    let temporally_stamped_fact_edges =
        count_temporally_stamped_edges(store.as_ref(), paper, &citation_nodes, "CITES").await
            + count_temporally_stamped_edges(
                store.as_ref(),
                paper,
                &quality_nodes,
                "hasQualitySignal",
            )
            .await;
    let import_eligible = graph_import_eligible(store.as_ref()).await?;
    let endpoint_triples = after
        .edges
        .iter()
        .map(|edge| {
            format!(
                "{}:{}->{}={}",
                edge.edge_type, edge.source_label, edge.target_label, edge.count
            )
        })
        .collect::<Vec<_>>();
    let endpoint_replay_stable = before_target_edges == target_endpoint_projection(&after);
    let full_graph_replay_stable = before == after;

    anyhow::ensure!(citation_nodes.len() == 2, "expected two Citation nodes");
    anyhow::ensure!(
        quality_nodes.len() == 2,
        "expected two independent QualitySignal fixtures"
    );
    anyhow::ensure!(cites_edges == 2, "expected two CITES edges");
    anyhow::ensure!(quality_signal_edges == 2, "expected two quality edges");
    anyhow::ensure!(
        temporally_stamped_fact_edges == 4,
        "citation and independent quality fact edges lack temporal stamps"
    );
    anyhow::ensure!(
        forbidden_paper_to_paper_cites == 0,
        "Paper->Paper CITES drift"
    );
    anyhow::ensure!(
        forbidden_summary_to_quality_signal == 0,
        "hasSummary must not target QualitySignal"
    );
    anyhow::ensure!(!import_eligible, "D127 violation in canary graph");
    anyhow::ensure!(replay.cites_edges_written == 0, "CITES replay wrote edges");
    anyhow::ensure!(
        endpoint_replay_stable,
        "target endpoint graph changed on replay"
    );
    anyhow::ensure!(
        full_graph_replay_stable,
        "full canary graph changed on replay"
    );

    let node_schema_source = include_bytes!("../../../data/ontology/node_schemas.yaml");
    let edge_contract_source = include_bytes!("../../../data/ontology/edge_contracts.yaml");
    let task_source =
        include_str!("../../../data/graph_tasks/ontology_v2_endpoint_canary.tasks.cypher");
    let task_suite = parse_tasks_cypher(task_source).map_err(anyhow::Error::msg)?;
    let semantic_tasks =
        run_graph_task_suite(store.as_ref(), "ontology_v2_canary", &task_suite).await;
    anyhow::ensure!(
        semantic_tasks.passed(),
        "endpoint semantic task suite failed: {}",
        serde_json::to_string(&semantic_tasks)?
    );

    let policy_source = include_str!("../../../data/ontology/finalize_policy.yaml");
    let policy: FinalizePolicy = serde_yaml::from_str(policy_source)?;
    let policy_fingerprint = format!(
        "sha256:{}",
        ContentDigest::of_bytes(policy_source.as_bytes()).as_str()
    );
    let registry = kg_ontology::OntologyRegistry::load_from_dir("data/ontology")?;
    let finalizer = GraphFinalizeUseCase::new(policy, policy_fingerprint, registry)
        .map_err(anyhow::Error::msg)?;
    let pipeline = PipelineRunReport {
        pipeline_id: "ontology-v2-endpoint-canary".into(),
        article_id: paper_id.into(),
        completed: false,
        stages: vec![StageRunRecord {
            step_name: "semantic_enrichment".into(),
            stage_id: "semantic_enrichment".into(),
            resolved_from: None,
            cache_key_sha256: None,
            admission: None,
            disposition: StageDisposition::Executed {
                output_sha256: format!(
                    "sha256:{}",
                    ContentDigest::of_bytes(&serde_json::to_vec(&after)?).as_str()
                ),
            },
        }],
    };
    let finalize = finalizer
        .finalize(
            direct_store.as_ref(),
            &pipeline,
            semantic_tasks.classified_evidence(std::iter::empty()),
        )
        .await;
    let finalize_blockers = finalize
        .findings
        .iter()
        .map(|finding| format!("{}:{}", finding.code, finding.evidence))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        finalize.schema_violations == 0,
        "endpoint graph has schema violations: {:?}",
        finalize.findings
    );
    anyhow::ensure!(
        finalize.unknown_edge_types.is_empty(),
        "endpoint graph has unknown edge types"
    );
    anyhow::ensure!(
        finalize.import_eligible_true == 0 && !finalize.import_eligible,
        "D127 drift in finalize"
    );
    anyhow::ensure!(
        !finalize_blockers.is_empty()
            && finalize_blockers
                .iter()
                .all(|code| code.starts_with("required_stage_not_passed:")),
        "unexpected finalize blockers: {:?}",
        finalize.findings
    );

    let graph_semantics_sha256 = format!(
        "sha256:{}",
        ContentDigest::of_bytes(&serde_json::to_vec(&after)?).as_str()
    );
    let report = OntologyEndpointEvidence {
        schema_version: ENDPOINT_EVIDENCE_SCHEMA_VERSION.into(),
        paper_id: paper_id.into(),
        graph_semantics_sha256,
        node_schemas_sha256: format!(
            "sha256:{}",
            ContentDigest::of_bytes(node_schema_source).as_str()
        ),
        edge_contracts_sha256: format!(
            "sha256:{}",
            ContentDigest::of_bytes(edge_contract_source).as_str()
        ),
        finalize_policy_sha256: format!(
            "sha256:{}",
            ContentDigest::of_bytes(policy_source.as_bytes()).as_str()
        ),
        semantic_tasks_source_sha256: format!(
            "sha256:{}",
            ContentDigest::of_bytes(task_source.as_bytes()).as_str()
        ),
        first_cites_edges_written: first.cites_edges_written,
        replay_cites_edges_written: replay.cites_edges_written,
        citation_nodes: citation_nodes.len(),
        quality_signal_nodes: quality_nodes.len(),
        cites_edges,
        quality_signal_edges,
        forbidden_paper_to_paper_cites,
        forbidden_summary_to_quality_signal,
        temporally_stamped_fact_edges,
        endpoint_triples,
        endpoint_replay_stable,
        full_graph_replay_stable,
        semantic_task_count: semantic_tasks.tasks.len(),
        semantic_tasks_passed: semantic_tasks.passed(),
        finalize_passed: finalize.verdict == FinalizeVerdict::Pass,
        finalize_schema_violations: finalize.schema_violations,
        finalize_unknown_edge_types: finalize.unknown_edge_types,
        finalize_blockers,
        import_eligible,
        report_sha256: String::new(),
    }
    .seal()?;
    atomic_write(&args.report, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "ontology_endpoint_canary citations={} quality_signals={} endpoint_replay_stable={} full_graph_replay_stable={} import_eligible=false",
        report.citation_nodes,
        report.quality_signal_nodes,
        report.endpoint_replay_stable,
        report.full_graph_replay_stable
    );
    Ok(())
}

fn parsed(paper_id: &str) -> ParsedArticle {
    ParsedArticle {
        paper_id: paper_id.into(),
        title: "Ontology endpoint canary".into(),
        abstract_text: "Abstract".into(),
        body_text: "Deterministic endpoint canary.".into(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: None,
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        pdf_hash: "fixture-sha256".into(),
    }
}

fn target_endpoint_projection(
    observation: &da_application::ontology_contract_audit::RuntimeGraphAuditObservation,
) -> Vec<(String, String, String, usize)> {
    observation
        .edges
        .iter()
        .filter(|edge| matches!(edge.edge_type.as_str(), "CITES" | "hasQualitySignal"))
        .map(|edge| {
            (
                edge.edge_type.clone(),
                edge.source_label.clone(),
                edge.target_label.clone(),
                edge.count,
            )
        })
        .collect()
}

async fn count_temporally_stamped_edges(
    store: &(dyn DirectGraphStore + 'static),
    source: u64,
    targets: &[u64],
    edge_type: &str,
) -> usize {
    let mut count = 0;
    for target in targets {
        for (edge, _) in store.get_edges_between(source, *target, edge_type).await {
            if store
                .get_edge_property_string(edge, "created_at")
                .await
                .is_some()
                && store
                    .get_edge_property_string(edge, "valid_at")
                    .await
                    .is_some()
            {
                count += 1;
            }
        }
    }
    count
}

async fn materialize_quality_signal_fixtures(
    store: &(dyn DirectGraphStore + 'static),
    paper: u64,
    paper_id: &str,
) -> Result<()> {
    for (signal_type, text) in [
        ("reproducibility", "Deterministic fixture evidence."),
        ("reading_difficulty", "Bounded fixture evidence."),
    ] {
        let vid = format!("vid:quality:{paper_id}:{signal_type}");
        let node = match store
            .find_node_by_string_property("QualitySignal", "vid", &vid)
            .await
        {
            Some(node) => node,
            None => store.create_node("QualitySignal").await?,
        };
        store.set_node_property_string(node, "vid", vid).await?;
        store
            .set_node_property_string(node, "text", text.into())
            .await?;
        store
            .set_node_property_string(node, "signal_type", signal_type.into())
            .await?;
        store
            .set_node_property_string(node, "method", "deterministic_canary_fixture".into())
            .await?;
        store
            .set_node_property_bool(node, "retrieval_eligible", false)
            .await?;
        store
            .set_node_property_bool(node, "import_eligible", false)
            .await?;
        store
            .set_node_property_int(node, "schema_version", 1)
            .await?;
        // PIR-008 / M398-S04: canonical RFC3339 UTC string wire form (fixed
        // deterministic instant, mirroring the edge created_at/valid_at).
        store
            .set_node_property_string(node, "valid_from", "2023-11-14T22:13:20Z".to_string())
            .await?;
        if store
            .get_edges_between(paper, node, "hasQualitySignal")
            .await
            .is_empty()
        {
            let edge = store.create_edge(paper, node, "hasQualitySignal").await?;
            store
                .set_edge_property_string(edge, "created_at", "2023-11-14T22:13:20Z")
                .await?;
            store
                .set_edge_property_string(edge, "valid_at", "2023-11-14T22:13:20Z")
                .await?;
        }
    }
    Ok(())
}

async fn graph_import_eligible(store: &(dyn DirectGraphStore + 'static)) -> Result<bool> {
    for label in ["Paper", "Section", "Citation", "Summary", "QualitySignal"] {
        for node in store.get_nodes_by_label(label).await {
            let value = store
                .get_node_property_bool(node, "import_eligible")
                .await
                .with_context(|| format!("missing import_eligible on {label} node {node}"))?;
            if value {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

async fn count_edges(
    store: &(dyn DirectGraphStore + 'static),
    source: u64,
    targets: &[u64],
    edge_type: &str,
) -> usize {
    let mut count = 0;
    for target in targets {
        count += store
            .get_edges_between(source, *target, edge_type)
            .await
            .len();
    }
    count
}

fn ensure_report_path(path: &Path) -> Result<()> {
    let root = std::env::current_dir()?.canonicalize()?;
    let artifact_root = root.join("artifacts/ontology");
    std::fs::create_dir_all(&artifact_root)?;
    let parent = path
        .parent()
        .context("report parent missing")?
        .canonicalize()?;
    anyhow::ensure!(
        parent == artifact_root.canonicalize()?,
        "report path escapes artifacts/ontology"
    );
    Ok(())
}
