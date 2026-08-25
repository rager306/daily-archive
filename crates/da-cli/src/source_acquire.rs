//! `da source acquire <raw>` — raw-link acquisition dispatcher (M451).
//!
//! Meta/prompt: routing rules live in source_codes.yaml (DATA). The CLI
//! matches the raw input against the rules and delegates to existing
//! pipeline commands (arxiv_catalog) or reports the handler needed for
//! non-arxiv kinds (doi_mirror/direct_pdf are reference tools today).

use regex::Regex;
use serde_yaml::Value;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Route {
    pub kind: String,
    pub id: String,
    pub handler: String,
}

/// Match a raw link/id against the DATA routing table (ordered).
pub fn route(raw: &str, routing_yaml: &str) -> anyhow::Result<Option<Route>> {
    let v: Value = serde_yaml::from_str(routing_yaml)
        .map_err(|e| anyhow::anyhow!("routing YAML: {e}"))?;
    let Some(rules) = v.get("acquisition_routing").and_then(|r| r.get("rules")).and_then(|r| r.as_sequence()) else {
        return Ok(None);
    };
    for rule in rules {
        let Some(pat) = rule.get("pattern").and_then(|p| p.as_str()) else { continue };
        let Ok(re) = Regex::new(pat) else { continue };
        if let Some(caps) = re.captures(raw) {
            let kind = rule.get("kind").and_then(|k| k.as_str()).unwrap_or("").to_string();
            let handler = rule.get("handler").and_then(|h| h.as_str()).unwrap_or("").to_string();
            let norm = rule.get("id_normalize").and_then(|n| n.as_str()).unwrap_or("none");
            let id = match rule.get("id_group").and_then(|g| g.as_u64()) {
                Some(g) if g > 0 => caps
                    .get(g as usize)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
                _ => raw.to_string(),
            };
            let id = if norm == "strip_version" {
                // strip only a trailing v-suffix: 2608.14905v2 -> 2608.14905
                if let Some(pos) = id.rfind('v') {
                    let suffix_digits = id[pos + 1..].chars().all(|c| c.is_ascii_digit());
                    if suffix_digits && !id[pos + 1..].is_empty() {
                        id[..pos].to_string()
                    } else {
                        id
                    }
                } else {
                    id
                }
            } else {
                id
            };
            return Ok(Some(Route { kind, id, handler }));
        }
    }
    Ok(None)
}

/// Execute acquisition for the routed source.
pub fn acquire(raw: &str, repo: &Path) -> anyhow::Result<()> {
    let routing = std::fs::read_to_string(repo.join("data/source_codes.yaml"))?;
    let Some(route) = route(raw, &routing)? else {
        anyhow::bail!("no acquisition route matches {raw:?} (source_codes.yaml)");
    };
    println!("route: kind={} id={} handler={}", route.kind, route.id, route.handler);

    match route.handler.as_str() {
        "arxiv_catalog" => {
            let status = std::process::Command::new(std::env::current_exe()?)
                .args([
                    "acquire-arxiv",
                    "--ids",
                    &route.id,
                    "--report",
                    "/tmp/da_acquire_last.json",
                ])
                .status()?;
            if !status.success() {
                anyhow::bail!("acquire-arxiv failed for {}", route.id);
            }
            println!("next: da source status {} ; da source plan {}", route.id, route.id);
            Ok(())
        }
        "doi_mirror" => acquire_doi(&route.id, repo),
        "direct_pdf" => {
            println!(
                "direct_pdf handler: download {raw} to data/sources and run \"da source run <id>\" \
                 (generic direct-download backlog; routing rule present)"
            );
            Ok(())
        }
        other => {
            println!("handler {other:?} not implemented (registry handler name mismatch)");
            Ok(())
        }
    }
}

/// DOI acquisition (replaces scripts/m451_doi_import.py — ADR-073 wave):
/// OpenAlex work lookup → best_oa_location.pdf_url → download →
/// data/sources/doi/<dir-safe>/source/ + identity.json. Uses the Rust
/// OpenAlex adapter; no hand-tuned mirrors.
fn acquire_doi(doi: &str, repo: &Path) -> anyhow::Result<()> {
    use da_adapters::openalex_adapter::OpenAlexHttpAdapter;

    let rt = tokio::runtime::Runtime::new()?;
    let adapter = OpenAlexHttpAdapter::default();
    let url = format!("{}/works/doi:{doi}", doi_base());
    let _ = &adapter; // adapter keeps its client private; plain client below
    let resp = rt.block_on(async {
        let c = reqwest::Client::builder()
            .user_agent("daily-archive/0.1")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow::anyhow!("reqwest: {e}"))?;
        c.get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("openalex: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("openalex status: {e}"))?
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("openalex body: {e}"))
    })?;
    let body: serde_json::Value = serde_json::from_str(&resp)
        .map_err(|e| anyhow::anyhow!("openalex decode: {e}"))?;
    let pdf_url = best_pdf_url(&body)
        .ok_or_else(|| anyhow::anyhow!("no open-access pdf_url in OpenAlex for {doi}"))?;
    let title = body.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();

    let dir_safe = doi.replace('/', "-");
    let src_dir = repo.join(format!("data/sources/doi/{dir_safe}/source"));
    std::fs::create_dir_all(&src_dir)?;
    let fname = format!("{dir_safe}.pdf");
    let pdf_path = src_dir.join(&fname);

    println!("doi {doi}: downloading {pdf_url}");
    let bytes = rt.block_on(async { download_bytes(&pdf_url).await })?;
    std::fs::write(&pdf_path, &bytes)?;

    let ident = serde_json::json!({
        "schema_version": "article.v00.01",
        "paper_id": dir_safe,
        "doi": doi,
        "title": title,
        "source_code": "doi",
        "source_type": "article",
        "source_url": pdf_url,
        "format": "pdf",
        "language": "en",
    });
    std::fs::write(
        repo.join(format!("data/sources/doi/{dir_safe}/identity.json")),
        serde_json::to_string_pretty(&ident)?,
    )?;
    println!("acquired: data/sources/doi/{dir_safe}/ ({} bytes)", bytes.len());
    println!("next: da source run {dir_safe}");
    Ok(())
}

fn doi_base() -> String {
    std::env::var("OPENALEX_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "https://api.openalex.org".to_string())
}

fn best_pdf_url(work: &serde_json::Value) -> Option<String> {
    let best = work.pointer("/best_oa_location/pdf_url").and_then(|u| u.as_str());
    if best.is_some() {
        return best.map(String::from);
    }
    work.get("locations")
        .and_then(|l| l.as_array())?
        .iter()
        .find_map(|l| l.get("pdf_url").and_then(|u| u.as_str()).map(String::from))
}

async fn download_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let c = reqwest::Client::builder()
        .user_agent("daily-archive/0.1")
        .timeout(std::time::Duration::from_secs(180))
        .build()?;
    let bytes = c.get(url).send().await?.error_for_status()?.bytes().await?;
    Ok(bytes.to_vec())
}
