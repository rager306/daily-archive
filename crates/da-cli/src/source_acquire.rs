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
                    "--allow-network",
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
        other => {
            // honest: reference handlers are surfaced, not silently run
            println!(
                "handler {other:?} is a reference tool (phase-2 backlog); \
                 use scripts/m451_doi_import.py or place assets manually"
            );
            Ok(())
        }
    }
}
