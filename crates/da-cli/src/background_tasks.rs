//! `da source background-run <task> [pid|pending]` — background task runner
//! (ADR-073 wave: replaces scripts/m451_background_runner.py; the registry
//! data/ontology/background_tasks.yaml stays the single policy source).
//!
//! Implements the per_section_summaries task: batch sections (≤16/request),
//! scrub injections (DATA policy), LLM via models.yaml binding, cache by
//! fingerprint {pid, model, prompt_sha256, sections_sha256}, rate limit and
//! daily budget from the task policy, output summaries/per_section.json +
//! manifest artifact marker. Idempotent reruns.

use da_adapters::model_registry::load_model_registry;
use da_adapters::openai_compatible_llm::OpenAiCompatibleLlmClient;
use da_domain::summaries_llm::{compile_injection_policy, scrub_injection};
use da_ports::llm_client::{ChatMessage, ChatOptions};

const CACHE_DIR: &str = "artifacts/pipeline_cache/bg_per_section";
const STATE_FILE: &str = "artifacts/pipeline_cache/bg_state.json";

struct TaskSpec {
    model_binding: String,
    prompt_file: String,
    min_section_chars: usize,
    max_sections_per_request: usize,
    requests_per_min: u32,
    daily_budget: u32,
}

fn load_task(task_id: &str) -> anyhow::Result<TaskSpec> {
    let yaml = std::fs::read_to_string("data/ontology/background_tasks.yaml")?;
    let v: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
    let task = v
        .get("tasks")
        .and_then(|t| t.as_sequence())
        .and_then(|s| s.iter().find(|t| t.get("id").and_then(|i| i.as_str()) == Some(task_id)))
        .ok_or_else(|| anyhow::anyhow!("task {task_id} not in background_tasks.yaml"))?;
    if !task.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false) {
        anyhow::bail!("task {task_id} disabled in registry");
    }
    let policy = task.get("policy").cloned().unwrap_or_default();
    let p = |k: &str| -> anyhow::Result<usize> {
        policy
            .get(k)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .ok_or_else(|| anyhow::anyhow!("policy.{k} missing"))
    };
    Ok(TaskSpec {
        model_binding: task
            .get("model_binding")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string(),
        prompt_file: task
            .get("prompt_file")
            .and_then(|m| m.as_str())
            .unwrap_or_default()
            .to_string(),
        min_section_chars: p("min_section_chars")?,
        max_sections_per_request: p("max_sections_per_request")?,
        requests_per_min: p("requests_per_min")? as u32,
        daily_budget: p("daily_budget_requests")? as u32,
    })
}

fn sha256_hex(s: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s);
    hex::encode(h.finalize())
}

fn bg_state_bump(state_path: &std::path::Path) -> anyhow::Result<u32> {
    let today = chrono_now()[..10].to_string();
    let mut state: serde_json::Value = std::fs::read_to_string(state_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let used = if state.get("date").and_then(|d| d.as_str()) == Some(&today) {
        state["used"].as_u64().unwrap_or(0) as u32
    } else {
        0
    };
    let used = used + 1;
    state["date"] = serde_json::Value::String(today);
    state["used"] = serde_json::Value::from(used);
    std::fs::write(state_path, serde_json::to_string(&state)?)?;
    Ok(used)
}

fn extract_json_array(content: &str) -> Option<serde_json::Value> {
    // 9router gotchas (python parity): <think> blocks, markdown fences
    let cleaned = if let Some(a) = content.find("<think>") {
        let tail = &content[a..];
        let end = tail.find("</think>").map(|e| a + e + 8).unwrap_or(content.len());
        &content[end..]
    } else {
        content
    };
    let cleaned = cleaned.trim();
    let cleaned = cleaned
        .strip_prefix("```json")
        .or_else(|| cleaned.strip_prefix("```"))
        .unwrap_or(cleaned)
        .trim();
    let cleaned = cleaned
        .strip_suffix("```")
        .map(str::trim)
        .unwrap_or(cleaned);
    // адаптер требует response_format=json_object → модель может отдать
    // объект-обёртку {"sections": [...]}: ищем массив внутри значений
    if let Some(arr) = cleaned
        .find('[')
        .and_then(|start| cleaned.rfind(']').map(|end| &cleaned[start..=end]))
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .filter(|v| v.is_array())
    {
        return Some(arr);
    }
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(cleaned)
        && let Some(map) = obj.as_object()
    {
        for v in map.values() {
            if v.is_array() {
                return Some(v.clone());
            }
        }
    }
    // поток склеенных объектов (по одному на секцию):
    // {..}
    // {..}
    let stream: Result<Vec<_>, _> =
        serde_json::Deserializer::from_str(cleaned).into_iter::<serde_json::Value>().collect();
    if let Ok(items) = stream
        && !items.is_empty()
    {
        return Some(serde_json::Value::Array(items));
    }
    None
}

/// Run the task for one article (cache-idempotent).
/// Run-context bundle (clippy arity discipline).
struct RunCtx<'a> {
    llm: &'a OpenAiCompatibleLlmClient,
    model: &'a str,
    prompt: &'a str,
    prompt_sha: &'a str,
    patterns: &'a [(regex::Regex, String)],
    state_path: std::path::PathBuf,
    rt: &'a tokio::runtime::Runtime,
}

fn run_per_section(pid: &str, spec: &TaskSpec, ctx: &RunCtx<'_>) -> anyhow::Result<&'static str> {
    let (llm, model, prompt, prompt_sha, patterns, state_path, rt) =
        (&ctx.llm, &ctx.model, &ctx.prompt, &ctx.prompt_sha, &ctx.patterns, &ctx.state_path, &ctx.rt);
    use da_ports::llm_client::LLMClient as _;
    let cache_dir = std::path::Path::new(CACHE_DIR);
    std::fs::create_dir_all(cache_dir)?;
    let cf = cache_dir.join(format!("{pid}.json"));

    let article_dir = std::path::Path::new("data/processed").join(pid);
    let nav: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(article_dir.join("context/navigation.json"))?)?;
    // long sections only (policy); navigation carries path_id/section_id/title
    let sections: Vec<&serde_json::Value> = nav
        .iter()
        .filter(|n| n.get("char_count").and_then(|c| c.as_u64()).unwrap_or(0) as usize
            >= spec.min_section_chars)
        .collect();
    if sections.is_empty() {
        return Ok("no_sections");
    }
    let sections_sha = sha256_hex(
        sections
            .iter()
            .map(|n| n.get("section_id").and_then(|s| s.as_str()).unwrap_or(""))
            .collect::<Vec<_>>()
            .join(",")
            .as_bytes(),
    );

    // cache fingerprint (registry cache_fingerprint fields)
    if let Ok(prev) = std::fs::read_to_string(&cf)
        && let Ok(p) = serde_json::from_str::<serde_json::Value>(&prev)
    {
            let same = p.get("model").and_then(|m| m.as_str()) == Some(model)
                && p.get("prompt_sha256").and_then(|s| s.as_str()) == Some(prompt_sha)
                && p.get("sections_sha256").and_then(|s| s.as_str()) == Some(&sections_sha)
                && p.get("ok").and_then(|o| o.as_bool()) == Some(true);
        if same {
            return Ok("cache");
        }
    }

    // batches of ≤ max_sections_per_request
    let mut summaries: Vec<serde_json::Value> = Vec::new();
    let mut scrubbed_total = 0usize;
    let mut ok_all = true;
    let rate_delay = std::time::Duration::from_secs(60 / spec.requests_per_min.max(1) as u64);
    for batch in sections.chunks(spec.max_sections_per_request.max(1)) {
        if bg_state_bump(state_path)? > spec.daily_budget {
            ok_all = false;
            break;
        }
        let user_raw = batch
            .iter()
            .map(|n| {
                format!(
                    "[path_id={} section_id={} title={}]\n{}",
                    n.get("path_id").and_then(|v| v.as_str()).unwrap_or(""),
                    n.get("section_id").and_then(|v| v.as_str()).unwrap_or(""),
                    n.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    n.get("text_preview").and_then(|v| v.as_str()).unwrap_or(""),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let (user, scrubbed) = scrub_injection(&user_raw, patterns);
        scrubbed_total += scrubbed;
        let user: String = user.chars().take(24000).collect();

        let resp = rt.block_on(llm.chat(
            &[
                ChatMessage { role: "system".into(), content: prompt.to_string() },
                ChatMessage { role: "user".into(), content: user },
            ],
            &ChatOptions { model: model.to_string(), temperature: 0.2, max_tokens: 4000 },
        ));
        match resp {
            Ok(r) => {
                match extract_json_array(&r.text) {
                Some(arr) if arr.is_array() => summaries.extend(
                    arr.as_array().cloned().unwrap_or_default(),
                ),
                _ => { ok_all = false; break; }
                }
            }
            Err(_) => { ok_all = false; break; }
        }
        std::thread::sleep(rate_delay);
    }

    let rec = serde_json::json!({
        "pid": pid, "model": model, "prompt_sha256": prompt_sha,
        "sections_sha256": sections_sha, "ok": ok_all,
        "summaries": summaries,
        "sections_meta": sections.iter().map(|n| serde_json::json!({
            "path_id": n.get("path_id"), "section_id": n.get("section_id"),
            "title": n.get("title"),
        })).collect::<Vec<_>>(),
        "fetched_at": chrono_now(), "injection_scrubbed": scrubbed_total,
    });
    if ok_all {
        std::fs::write(&cf, serde_json::to_string(&rec)?)?;
    }

    // output: summaries/per_section.json + manifest marker
    let out_dir = article_dir.join("summaries");
    std::fs::create_dir_all(&out_dir)?;
    let out_json = serde_json::to_string_pretty(&rec)?;
    std::fs::write(out_dir.join("per_section.json"), &out_json)?;
    let mp = article_dir.join("manifest.yaml");
    let mut m: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(&mp)?)?;
    if let Some(arts) = m.get_mut("artifacts").and_then(|a| a.as_mapping_mut()) {
        let mut sm = arts
            .get("summaries")
            .cloned()
            .and_then(|s| s.as_mapping().cloned())
            .unwrap_or_default();
        let marker = if ok_all {
            serde_yaml::Mapping::from_iter([
                ("status".into(), "built".into()),
                ("file".into(), "summaries/per_section.json".into()),
                ("derived_from".into(), "parsed".into()),
                ("sha256".into(), sha256_hex(out_json.as_bytes()).into()),
                ("actor".into(), "background-rust".into()),
            ])
        } else {
            serde_yaml::Mapping::from_iter([
                ("status".into(), "failed".into()),
                ("reason".into(), "llm_batch_failed".into()),
            ])
        };
        sm.insert("per_section".into(), marker.into());
        arts.insert("summaries".into(), sm.into());
    }
    std::fs::write(&mp, serde_yaml::to_string(&m)?)?;
    Ok(if ok_all { "built" } else { "failed" })
}

pub fn run_background(task_id: &str, target: &str) -> anyhow::Result<()> {
    let spec = load_task(task_id)?;
    if spec.model_binding.is_empty() || spec.prompt_file.is_empty() {
        anyhow::bail!("task {task_id}: model_binding/prompt_file missing in registry");
    }
    let registry = load_model_registry(std::path::Path::new("models.yaml"))
        .map_err(|e| anyhow::anyhow!("models.yaml: {e:?}"))?;
    let binding = registry
        .resolve(&spec.model_binding, None)
        .map_err(|e| anyhow::anyhow!("binding {}: {e}", spec.model_binding))?;
    let key = std::env::var("NINE_ROUTER_API_KEY").ok().filter(|k| !k.trim().is_empty());
    let llm = OpenAiCompatibleLlmClient::new(binding.clone(), key, 240)
        .map_err(|e| anyhow::anyhow!("llm client: {e}"))?;
    let prompt = std::fs::read_to_string(&spec.prompt_file)?;
    let prompt_sha = sha256_hex(prompt.as_bytes());
    let injection_yaml = std::fs::read_to_string("data/ontology/prompt_injection_policy.yaml")?;
    let patterns = compile_injection_policy(&injection_yaml);

    let state_path = std::path::Path::new(STATE_FILE);
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let pids: Vec<String> = if target == "pending" || target.is_empty() {
        let mut out = Vec::new();
        for e in std::fs::read_dir("data/processed")? {
            let e = e?;
            if e.path().join("parsed/article.json").is_file() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
        out.sort();
        out
    } else {
        vec![target.to_string()]
    };

    let rt = tokio::runtime::Runtime::new()?;
    let mut counts = std::collections::BTreeMap::new();
    for pid in &pids {
        let ctx = RunCtx {
            llm: &llm,
            model: &binding.model_name,
            prompt: &prompt,
            prompt_sha: &prompt_sha,
            patterns: &patterns,
            state_path: state_path.to_path_buf(),
            rt: &rt,
        };
        let status = run_per_section(pid, &spec, &ctx).unwrap_or("error");
        *counts.entry(status.to_string()).or_insert(0u32) += 1;
    }
    println!("background {task_id}: {counts:?}");
    Ok(())
}

fn chrono_now() -> String {
    // same format as source_cli stamps
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs() as i64;
            let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        })
        .unwrap_or_default()
}


/// SONA pattern extraction from CVF trajectories (M452 S01).
/// Reads trajectory.jsonl for the given paper, builds ReasoningBank,
/// prints extracted patterns with quality metrics.
pub fn run_sona_analyze(pid: &str) -> anyhow::Result<()> {
    let traj_path = std::path::Path::new("data/agent_memory")
        .join(pid)
        .join("trajectory.jsonl");
    let text = std::fs::read_to_string(&traj_path)
        .map_err(|e| anyhow::anyhow!("trajectory {}: {e}", traj_path.display()))?;

    let records: Vec<serde_json::Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if records.is_empty() {
        println!("sona-analyze: no trajectories found for {pid}");
        return Ok(());
    }

    let dim = 8;
    let k_clusters = (records.len() / 5).clamp(2, 10);
    let patterns = da_application::sona_bridge::build_reasoning_bank(&records, dim, k_clusters);

    println!(
        "== sona-analyze: {pid} ==\n{} trajectories, {} patterns extracted",
        records.len(),
        patterns.len()
    );
    println!("{}", da_application::sona_bridge::summarize_patterns(&patterns));
    Ok(())
}
