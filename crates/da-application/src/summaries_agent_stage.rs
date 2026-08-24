//! Summaries agent stage (M451 W1): LLM summary via the AGENT MANIFEST
//! (data/agents/<id>/manifest.yaml — DATA, no hardcoded models/prompts),
//! injection scrubbing, Trajectory recording into the CVF memory port.
//! ADR-055 lineage: model/binding/prompt_sha in provenance.

use da_domain::agent_manifest::AgentManifest;
use da_domain::summaries_llm::{
    build_user_message, compile_injection_policy, extract_json, grounding_score, scrub_injection,
    SummarySectionInput,
};
use da_ports::agent_memory::{AgentMemory, ArticleFeatures, TrajectoryRecord};
use da_ports::llm_client::{ChatMessage, ChatOptions, LLMClient};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum SummariesAgentError {
    #[error("summaries agent: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("summaries agent: parsed unreadable for {0}")]
    NoParsed(String),
    #[error("summaries agent: {0}")]
    Agent(String),
    #[error("summaries agent: LLM: {0}")]
    Llm(String),
    #[error("summaries agent: unparseable model output")]
    Unparseable,
}

#[derive(Debug, Clone)]
pub struct SummariesAgentOutput {
    pub paper_id: String,
    pub summary_node_json: String,
    pub prompt_sha256: String,
    pub scrubbed: usize,
    pub latency_ms: u64,
    pub epoch: Option<u64>,
    pub ok: bool,
    pub grounding: f64,
}

/// Bundled runtime context for one agent invocation (keeps the
/// signature under clippy limits; all DATA-driven).
pub struct AgentRunContext<'a> {
    pub store: &'a dyn SchemaObjectStore,
    pub memory: &'a mut dyn AgentMemory,
    pub llm: &'a dyn LLMClient,
    pub manifest: &'a AgentManifest,
    pub model_name: &'a str,
    pub system_prompt: &'a str,
    pub injection_policy_yaml: &'a str,
    pub paper_id: &'a str,
}

pub async fn run_summaries_agent(
    ctx: AgentRunContext<'_>,
) -> Result<SummariesAgentOutput, SummariesAgentError> {
    let AgentRunContext { store, memory, llm, manifest, model_name, system_prompt, injection_policy_yaml, paper_id } = ctx;
    manifest.check_locks().map_err(SummariesAgentError::Agent)?;
    if !manifest.identity.enabled {
        return Err(SummariesAgentError::Agent(format!(
            "agent {} disabled",
            manifest.agent_id
        )));
    }
    let prompt_sha = sha256_hex(system_prompt.as_bytes());
    let prompt_sha_traj = prompt_sha.clone();

    // parsed sections via the store
    let raw = store
        .read_raw(paper_id, "parsed/article.json")?
        .ok_or_else(|| SummariesAgentError::NoParsed(paper_id.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| SummariesAgentError::NoParsed(format!("{paper_id}: {e}")))?;
    let title = parsed.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let abstract_text = parsed
        .get("abstract_text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sections: Vec<SummarySectionInput> = serde_json::from_value(
        parsed.get("sections").cloned().unwrap_or_default(),
    )
    .map_err(|e| SummariesAgentError::NoParsed(format!("{paper_id}: sections: {e}")))?;
    let max_chars = manifest
        .prompt
        .inputs
        .get("max_body_chars")
        .and_then(|v| v.as_u64())
        .unwrap_or(24000) as usize;
    let user = build_user_message(title, abstract_text, &sections, max_chars);

    // guardrail: scrub injection (DATA)
    let policy = compile_injection_policy(injection_policy_yaml);
    let (user_clean, scrubbed) = scrub_injection(&user, &policy);

    // LLM call via the port (binding-resolved model)
    let t0 = std::time::Instant::now();
    let resp = llm
        .chat(
            &[
                ChatMessage { role: "system".into(), content: system_prompt.to_string() },
                ChatMessage { role: "user".into(), content: user_clean },
            ],
            &ChatOptions {
                model: model_name.to_string(),
                max_tokens: manifest.guardrails.max_output_tokens,
                temperature: 0.2,
            },
        )
        .await
        .map_err(|e| SummariesAgentError::Llm(e.to_string()))?;
    let latency_ms = t0.elapsed().as_millis() as u64;
    let obj = extract_json(&resp.text).ok_or(SummariesAgentError::Unparseable)?;
    let ok = resp.ok && obj.get("tldr").and_then(|v| v.as_str()).is_some();

    let node = json!({
        "schema_version": "summary-node.v1",
        "paper_id": paper_id,
        "tldr": obj.get("tldr"),
        "contributions": obj.get("contributions"),
        "methods": obj.get("methods"),
        "results": obj.get("results"),
        "limitations": obj.get("limitations"),
        "provenance": {
            "agent": manifest.agent_id,
            "model": model_name,
            "binding": manifest.model.generate_binding,
            "prompt_sha256": prompt_sha,
            "latency_ms": latency_ms,
            "injection_scrubbed": scrubbed,
            "tokens_out": resp.tokens_out,
        },
    });
    let node_json = serde_json::to_string_pretty(&node)
        .map_err(|e| SummariesAgentError::Agent(e.to_string()))?;

    // deterministic grounding (diagnostic, ACAP: separate from hard
    // invariants) + article features for routing decisions
    let summary_text = [
        obj.get("tldr").and_then(|v| v.as_str()).unwrap_or(""),
        obj.get("methods").and_then(|v| v.as_str()).unwrap_or(""),
        obj.get("results").and_then(|v| v.as_str()).unwrap_or(""),
        obj.get("limitations").and_then(|v| v.as_str()).unwrap_or(""),
        &obj.get("contributions")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "))
            .unwrap_or_default(),
    ].join(" ");
    let grounding = (grounding_score(&summary_text, &user) * 1000.0).round() / 1000.0;

    let body_chars: usize = sections.iter().map(|s| s.text.len()).sum();
    let formulas = parsed.get("formulas").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let furniture_share = store
        .read_raw(paper_id, "quality_signals.json")
        .ok()
        .flatten()
        .and_then(|q| serde_json::from_str::<serde_json::Value>(&q).ok())
        .and_then(|q| q.as_array().and_then(|arr| {
            arr.iter()
                .find(|s| s.get("signal_id").and_then(|v| v.as_str()) == Some("furniture_share"))
                .and_then(|s| s.get("value"))
                .and_then(|v| v.as_f64())
        }))
        .unwrap_or(0.0);
    let grammar = store
        .read_object(paper_id)
        .ok()
        .flatten()
        .and_then(|m| m.get("grammar_profile").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_else(|| "arxiv_pdf_dot".to_string());

    // trajectory into the CVF memory (record-only, W1)
    let rec = TrajectoryRecord {
        task_id: manifest.agent_id.clone(),
        paper_id: paper_id.to_string(),
        model: model_name.to_string(),
        prompt_sha256: prompt_sha_traj.clone(),
        ok,
        latency_ms,
        completion_tokens: resp.tokens_out,
        grounding_hint: Some(grounding),
        failure_pattern: None, // filled by critic/repair loop later
        article_features: Some(ArticleFeatures {
            grammar,
            body_chars,
            furniture_share,
            formulas_per_section: formulas as f64 / sections.len().max(1) as f64,
        }),
    };
    let epoch = if manifest.memory.trajectory && manifest.memory.backend == "cvf" {
        memory
            .record_trajectory(&rec)
            .map_err(|e| SummariesAgentError::Agent(e.to_string()))?
            .into()
    } else {
        None
    };

    Ok(SummariesAgentOutput {
        paper_id: paper_id.to_string(),
        summary_node_json: node_json,
        prompt_sha256: prompt_sha,
        scrubbed,
        latency_ms,
        epoch,
        ok,
        grounding,
    })
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::agent_memory::{AgentMemoryError, MemoryStats};
    use da_ports::llm_client::{LLMError, LLMResult};
    use std::collections::BTreeMap;

    struct FakeStore(BTreeMap<(String, String), String>);
    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str { "article" }
        fn norm(&self) -> Result<da_domain::schema_invariants::ObjectNorm, SchemaObjectStoreError> { unimplemented!() }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> { Ok(vec![]) }
        fn read_object(&self, _id: &str) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> { Ok(None) }
        fn read_collection(&self, _id: &str, _rel: &str) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> { Ok(None) }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> { Ok(true) }
        fn file_backed_checks(&self) -> Box<dyn da_domain::schema_invariants::FileBackedChecks + '_> { unimplemented!() }
        fn read_raw(&self, id: &str, rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(self.0.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    struct FakeLlm;
    #[async_trait::async_trait]
    impl LLMClient for FakeLlm {
        fn provider_id(&self) -> &str { "fake" }
        fn can_make_request(&self) -> bool { true }
        async fn chat(&self, _m: &[ChatMessage], _o: &ChatOptions) -> LLMResult<da_ports::llm_client::ChatResponse> {
            Ok(da_ports::llm_client::ChatResponse {
                text: r#"{"tldr":"core claim","contributions":["c1","c2"],"methods":"m","results":"r","limitations":"l"}"#.into(),
                ok: true, error: None, tokens_in: Some(10), tokens_out: Some(20),
            })
        }
        async fn extract_structured(&self, _t: &str, _v: &serde_json::Value) -> LLMResult<serde_json::Value> {
            Err(LLMError::LLM("unused".into()))
        }
    }

    struct FakeMemory(Option<TrajectoryRecord>);
    impl AgentMemory for FakeMemory {
        fn record_trajectory(&mut self, r: &TrajectoryRecord) -> Result<u64, AgentMemoryError> {
            self.0 = Some(r.clone());
            Ok(1)
        }
        fn verify(&self) -> bool { true }
        fn persist(&self) -> Result<(), AgentMemoryError> { Ok(()) }
        fn stats(&self) -> MemoryStats { MemoryStats { epochs: 1, trajectories: 1, last_epoch_in_ms: Some(1) } }
    }

    const MANIFEST: &str = "schema_version: agent-manifest.v1\nagent_id: summarizer\nagent_version: 1.0.0\nidentity: {role: summarizer, enabled: true}\nmodel: {generate_binding: paper-summary-generate-default}\nprompt: {system_file: data/prompts/summarizer/system.v1.md, inputs: {max_body_chars: 24000}}\nmemory: {backend: cvf, trajectory: true}\noptimization: {optimizer_enabled: false, candidate_slots: []}\nguardrails: {injection_policy: p, max_output_tokens: 4000, rate_limit_per_min: 12}\n";

    #[tokio::test]
    async fn agent_stage_end_to_end_fake() {
        let store = FakeStore(BTreeMap::from([(
            ("p".into(), "parsed/article.json".into()),
            r#"{"title":"T","abstract_text":"A","sections":[{"title":"S","text":"body"}]}"#.to_string(),
        )]));
        let mut mem = FakeMemory(None);
        let llm = FakeLlm;
        let m = AgentManifest::from_yaml_str(MANIFEST).unwrap();
        let out = run_summaries_agent(AgentRunContext {
            store: &store, memory: &mut mem, llm: &llm, manifest: &m,
            model_name: "agnes-ai/agnes-2.5-flash",
            system_prompt: "system prompt v1",
            injection_policy_yaml: "scrub_patterns: []\n",
            paper_id: "p",
        })
        .await
        .unwrap();
        assert!(out.ok);
        assert_eq!(out.epoch, Some(1));
        let rec = mem.0.as_ref().expect("trajectory recorded");
        assert!(rec.grounding_hint.is_some(), "grounding in trajectory");
        let f = rec.article_features.as_ref().expect("features in trajectory");
        assert_eq!(f.grammar, "arxiv_pdf_dot");
        assert!(out.grounding >= 0.0);
        let node: serde_json::Value = serde_json::from_str(&out.summary_node_json).unwrap();
        assert_eq!(node["provenance"]["agent"], "summarizer");
        assert_eq!(node["provenance"]["binding"], "paper-summary-generate-default");
        assert_eq!(node["tldr"], "core claim");
    }
}
