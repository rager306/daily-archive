//! Strict resolver for the repository canonical `models.yaml` registry.

use da_domain::{assurance::ContentDigest, prompt_optimization::ResolvedModelBinding};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRegistry {
    pub schema_version: String,
    pub models: Vec<ModelRecord>,
    pub bindings: Vec<ModelBinding>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRecord {
    pub id: String,
    pub provider: String,
    pub endpoint: String,
    pub model_name: String,
    pub tool_version: String,
    pub policy_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBinding {
    pub binding_id: String,
    pub model_id: String,
    pub description: String,
}
impl ModelRegistry {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "v0.1" || self.models.is_empty() || self.bindings.is_empty() {
            return Err("model registry schema/models/bindings invalid".into());
        }
        let models = self
            .models
            .iter()
            .map(|x| (x.id.as_str(), x))
            .collect::<BTreeMap<_, _>>();
        let bindings = self
            .bindings
            .iter()
            .map(|x| (x.binding_id.as_str(), x))
            .collect::<BTreeMap<_, _>>();
        if models.len() != self.models.len() || bindings.len() != self.bindings.len() {
            return Err("duplicate model or binding identity".into());
        }
        for m in &self.models {
            if [
                m.id.as_str(),
                m.provider.as_str(),
                m.endpoint.as_str(),
                m.model_name.as_str(),
                m.tool_version.as_str(),
                m.policy_version.as_str(),
            ]
            .into_iter()
            .any(|x| x.trim().is_empty())
            {
                return Err(format!("model {} incomplete", m.id));
            }
            if !matches!(m.provider.as_str(), "openai" | "anthropic") {
                return Err(format!("model {} provider unsupported", m.id));
            }
            let parsed = reqwest::Url::parse(&m.endpoint)
                .map_err(|e| format!("model {} endpoint invalid: {e}", m.id))?;
            if parsed.scheme() != "https" && !is_local_http(&parsed) {
                return Err(format!(
                    "model {} endpoint must be HTTPS or local HTTP",
                    m.id
                ));
            }
        }
        for b in &self.bindings {
            if b.binding_id.trim().is_empty()
                || b.description.trim().is_empty()
                || !models.contains_key(b.model_id.as_str())
            {
                return Err(format!("binding {} unresolved/incomplete", b.binding_id));
            }
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        binding_id: &str,
        endpoint_override: Option<&str>,
    ) -> Result<ResolvedModelBinding, String> {
        self.validate()?;
        let b = self
            .bindings
            .iter()
            .find(|x| x.binding_id == binding_id)
            .ok_or_else(|| format!("model binding {binding_id} not found"))?;
        let m = self
            .models
            .iter()
            .find(|x| x.id == b.model_id)
            .expect("registry validated");
        let protocol = match m.provider.as_str() {
            "openai" => "openai_chat_completions",
            "anthropic" => "anthropic_messages",
            _ => unreachable!(),
        };
        let endpoint = match endpoint_override {
            Some(base) if m.id.starts_with("ninerouter-") => join_ninerouter_endpoint(base)?,
            Some(_) => return Err("endpoint override is allowed only for ninerouter models".into()),
            None => m.endpoint.clone(),
        };
        let out = ResolvedModelBinding {
            binding_id: b.binding_id.clone(),
            model_id: m.id.clone(),
            provider: m.provider.clone(),
            protocol: protocol.into(),
            endpoint,
            model_name: m.model_name.clone(),
            tool_version: m.tool_version.clone(),
            policy_version: m.policy_version.clone(),
        };
        out.validate()?;
        Ok(out)
    }
    pub fn sha256(&self) -> Result<ContentDigest, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map(|x| ContentDigest::of_bytes(&x))
            .map_err(|e| e.to_string())
    }
}
pub fn validate_prompt_model_bindings(
    models: &ModelRegistry,
    prompts: &crate::prompt_task_registry::PromptTaskRegistry,
) -> Result<(), String> {
    models.validate()?;
    prompts.validate()?;
    for route in &prompts.routes {
        models.resolve(&route.model_binding_id, None)?;
    }
    Ok(())
}

pub fn load_model_registry(path: &Path) -> Result<ModelRegistry, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("stat model registry {}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err(format!(
            "model registry {} is not a bounded file",
            path.display()
        ));
    }
    let registry: ModelRegistry = serde_yaml::from_slice(
        &std::fs::read(path).map_err(|e| format!("read model registry {}: {e}", path.display()))?,
    )
    .map_err(|e| format!("parse model registry {}: {e}", path.display()))?;
    registry.validate()?;
    Ok(registry)
}
fn is_local_http(url: &reqwest::Url) -> bool {
    url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))
}
fn join_ninerouter_endpoint(base: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(base).map_err(|e| format!("NINEROUTER_URL invalid: {e}"))?;
    if url.scheme() != "https" && !is_local_http(&url) {
        return Err("NINEROUTER_URL must be HTTPS or local HTTP".into());
    }
    url.set_path("/v1/chat/completions");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_registry_resolves_summary_roles_via_ninerouter() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models.yaml");
        let r = load_model_registry(&p).unwrap();
        let prompts = crate::prompt_task_registry::load_prompt_task_registry(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ontology/prompt_tasks.yaml"),
        )
        .unwrap();
        validate_prompt_model_bindings(&r, &prompts).unwrap();
        let d = r
            .resolve(
                "paper-summary-generate-default",
                Some("http://127.0.0.1:20128"),
            )
            .unwrap();
        assert_eq!(d.protocol, "openai_chat_completions");
        assert_eq!(d.model_name, "agnes-ai/agnes-2.0-flash");
        assert_eq!(d.endpoint, "http://127.0.0.1:20128/v1/chat/completions");
        assert_eq!(
            r.resolve(
                "prompt-eval-summary-minimax-m3",
                Some("http://127.0.0.1:20128")
            )
            .unwrap()
            .model_name,
            "minimax/MiniMax-M3"
        );
        assert_eq!(
            r.resolve(
                "prompt-eval-figure-minimax-m3",
                Some("http://127.0.0.1:20128")
            )
            .unwrap()
            .model_name,
            "minimax/MiniMax-M3"
        );
        assert_eq!(
            r.resolve("prompt-eval-summary-agnes", Some("http://127.0.0.1:20128"))
                .unwrap()
                .model_name,
            "agnes-ai/agnes-2.0-flash"
        );
        assert_eq!(
            r.resolve("prompt-eval-figure-qwen-vl", Some("http://127.0.0.1:20128"))
                .unwrap()
                .model_name,
            "novita/qwen/qwen3-vl-235b-a22b-instruct"
        );
        for id in [
            "paper-summary-generate-quality",
            "paper-summary-generate-fallback",
            "paper-summary-judge-primary",
            "paper-summary-judge-secondary",
        ] {
            assert!(r.resolve(id, None).is_ok());
        }
        assert_eq!(r.sha256().unwrap(), r.sha256().unwrap());
    }
    #[test]
    fn remote_http_and_nonrouter_override_fail() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models.yaml");
        let r = load_model_registry(&p).unwrap();
        assert!(
            r.resolve("paper-summary-generate-default", Some("http://example.com"))
                .is_err()
        );
        assert!(
            r.resolve("article-artifact-classify", Some("https://proxy.example"))
                .is_err()
        );
    }
}
