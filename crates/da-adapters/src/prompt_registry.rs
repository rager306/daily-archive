//! DATA loader for the unified prompt / meta-prompt registry (ADR-055, D557).
//!
//! `data/ontology/prompt_registry.yaml` declares each prompt-bearing form by
//! `(prompt_id, prompt_version)` and names whether it is an inline-template
//! prompt task or a file-based meta-prompt agent. The loader computes the
//! `prompt_sha256` / `schema_sha256` fingerprints from the referenced source
//! bytes at load time (D418 recompute-from-source), so an edit to a prompt or
//! schema cannot leave a stale fingerprint pinned in DATA.
//!
//! Fail-closed rules, all returning a named error string:
//! - the registry file must be a bounded regular file (`<= 1 MiB`);
//! - `deny_unknown_fields` plus [`PromptRegistry::validate`] reject schema
//!   drift, duplicate `(prompt_id, prompt_version)` and any D127 unlock;
//! - a `task` entry must resolve in `data/ontology/prompt_tasks.yaml` at the
//!   declared version, and its `output_schema_id` must exist;
//! - a `meta_prompt` entry must carry a repo-relative `source_ref` and
//!   `schema_ref`; both are canonicalized and must stay inside `root`, so a
//!   symlink cannot escape the workspace;
//! - a `meta_prompt` entry's `prompt_version` must equal the bound agent
//!   manifest's `agent_version`, and the manifest's `prompt.system_file` must
//!   be the entry's `source_ref`, so the registry cannot point at a different
//!   prompt than the agent actually loads.

use da_domain::{
    assurance::ContentDigest,
    prompt_registry::{PromptEntryKind, PromptRegistry, PromptRegistryEntry},
};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::prompt_task_registry::PromptTaskRegistry;

/// Upper bound on any single file this loader reads (registry, manifest, source).
pub const MAX_PROMPT_REGISTRY_BYTES: u64 = 1024 * 1024;

/// Canonical provenance prefix for a `task` entry's derived `source_ref`.
pub const PROMPT_TASK_SOURCE_PREFIX: &str = "data/ontology/prompt_tasks.yaml#";

/// DATA declaration form of the registry.
///
/// Deliberately omits `prompt_sha256` / `schema_sha256`: those are computed
/// from the referenced source bytes at load time, never pinned in DATA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRegistryDeclaration {
    pub schema_version: String,
    pub registry_id: String,
    pub d127_import_eligible: bool,
    pub entries: Vec<PromptRegistryDeclarationEntry>,
}

/// One declared prompt form in [`PromptRegistryDeclaration`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRegistryDeclarationEntry {
    pub prompt_id: String,
    pub prompt_version: String,
    pub kind: PromptEntryKind,
    /// `output_schema_id` for `task`, repo-relative schema file for `meta_prompt`.
    pub schema_ref: String,
    /// Repo-relative prompt file for `meta_prompt`; derived for `task`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

/// The subset of an agent manifest this loader binds a meta-prompt entry to.
#[derive(Debug, Deserialize)]
struct AgentManifestBinding {
    agent_version: String,
    prompt: AgentManifestPrompt,
}

#[derive(Debug, Deserialize)]
struct AgentManifestPrompt {
    system_file: String,
}

/// Load and resolve the unified prompt registry from DATA.
///
/// `registry_path` is the declaration file; `task_registry` is the already
/// loaded `prompt_tasks.yaml` registry used to resolve `task` entries; `root`
/// is the workspace root that repo-relative `source_ref` / `schema_ref`
/// references (and the bound agent manifests) are resolved against.
pub fn load_prompt_registry(
    registry_path: &Path,
    task_registry: &PromptTaskRegistry,
    root: &Path,
) -> Result<PromptRegistry, String> {
    let bytes = read_bounded_file(registry_path, "prompt registry")?;
    let declaration: PromptRegistryDeclaration = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("parse prompt registry {}: {error}", registry_path.display()))?;
    let root = root
        .canonicalize()
        .map_err(|error| format!("resolve prompt registry root {}: {error}", root.display()))?;
    let mut entries = Vec::with_capacity(declaration.entries.len());
    for entry in &declaration.entries {
        entries.push(resolve_entry(entry, task_registry, &root)?);
    }
    let registry = PromptRegistry {
        schema_version: declaration.schema_version,
        registry_id: declaration.registry_id,
        d127_import_eligible: declaration.d127_import_eligible,
        entries,
    };
    registry.validate().map_err(|error| error.to_string())?;
    Ok(registry)
}

fn resolve_entry(
    entry: &PromptRegistryDeclarationEntry,
    task_registry: &PromptTaskRegistry,
    root: &Path,
) -> Result<PromptRegistryEntry, String> {
    if [
        entry.prompt_id.as_str(),
        entry.prompt_version.as_str(),
        entry.schema_ref.as_str(),
    ]
    .into_iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(format!(
            "prompt registry entry {}@{} identity is incomplete",
            entry.prompt_id, entry.prompt_version
        ));
    }
    match entry.kind {
        PromptEntryKind::Task => {
            let task = task_registry.task_versioned(&entry.prompt_id, &entry.prompt_version)?;
            let prompt_sha256 = task.template_sha256()?;
            let schema_sha256 = task_registry
                .output_schema(&task.output_schema_id)?
                .sha256()?;
            Ok(PromptRegistryEntry {
                prompt_id: entry.prompt_id.clone(),
                prompt_version: entry.prompt_version.clone(),
                kind: PromptEntryKind::Task,
                prompt_sha256,
                schema_ref: entry.schema_ref.clone(),
                schema_sha256,
                source_ref: format!("{PROMPT_TASK_SOURCE_PREFIX}{}", task.task_id),
            })
        }
        PromptEntryKind::MetaPrompt => {
            let source_ref = entry.source_ref.as_deref().ok_or_else(|| {
                format!(
                    "prompt registry meta_prompt entry {}@{} requires source_ref",
                    entry.prompt_id, entry.prompt_version
                )
            })?;
            if source_ref.trim().is_empty() {
                return Err(format!(
                    "prompt registry meta_prompt entry {}@{} has a blank source_ref",
                    entry.prompt_id, entry.prompt_version
                ));
            }
            // Validate the declared paths before touching the manifest or bytes.
            let source_path = confined_file_path(root, source_ref, "prompt source")?;
            let schema_path = confined_file_path(root, &entry.schema_ref, "prompt schema")?;
            let manifest = load_agent_manifest_binding(root, &entry.prompt_id)?;
            if manifest.agent_version != entry.prompt_version {
                return Err(format!(
                    "prompt registry entry {}@{} disagrees with agent manifest agent_version {}",
                    entry.prompt_id, entry.prompt_version, manifest.agent_version
                ));
            }
            if manifest.prompt.system_file != source_ref {
                return Err(format!(
                    "prompt registry entry {}@{} source_ref {} disagrees with agent manifest prompt.system_file {}",
                    entry.prompt_id, entry.prompt_version, source_ref, manifest.prompt.system_file
                ));
            }
            let prompt_bytes = read_bounded_file(&source_path, "prompt source")?;
            let schema_bytes = read_bounded_file(&schema_path, "prompt schema")?;
            Ok(PromptRegistryEntry {
                prompt_id: entry.prompt_id.clone(),
                prompt_version: entry.prompt_version.clone(),
                kind: PromptEntryKind::MetaPrompt,
                prompt_sha256: ContentDigest::of_bytes(&prompt_bytes),
                schema_ref: entry.schema_ref.clone(),
                schema_sha256: ContentDigest::of_bytes(&schema_bytes),
                source_ref: source_ref.to_string(),
            })
        }
    }
}

fn load_agent_manifest_binding(
    root: &Path,
    prompt_id: &str,
) -> Result<AgentManifestBinding, String> {
    let reference = format!("data/agents/{prompt_id}/manifest.yaml");
    let path = confined_file_path(root, &reference, "agent manifest")?;
    let bytes = read_bounded_file(&path, "agent manifest")?;
    serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("parse agent manifest {reference}: {error}"))
}

fn read_bounded_file(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("stat {what} {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_PROMPT_REGISTRY_BYTES {
        return Err(format!("{what} {} is not a bounded file", path.display()));
    }
    std::fs::read(path).map_err(|error| format!("read {what} {}: {error}", path.display()))
}

/// Resolve a repo-relative reference inside `root`.
///
/// Refuses an absolute path or any component that is not a plain directory name
/// (`..`, `/`, a Windows prefix), then canonicalizes the result and requires it
/// to still live under the canonical `root`, so a symlink cannot escape.
fn confined_file_path(root: &Path, reference: &str, what: &str) -> Result<PathBuf, String> {
    let relative = Path::new(reference);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{what} reference {reference} must be a repo-relative path inside the workspace root"
        ));
    }
    let resolved = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("resolve {what} {reference}: {error}"))?;
    if !resolved.starts_with(root) {
        return Err(format!(
            "{what} reference {reference} escapes the workspace root"
        ));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn real_task_registry() -> PromptTaskRegistry {
        crate::prompt_task_registry::load_prompt_task_registry(
            &repo_root().join("data/ontology/prompt_tasks.yaml"),
        )
        .unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("da-prompt-registry-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn registry_file(dir: &Path, entries: &str) -> PathBuf {
        let path = dir.join("prompt_registry.yaml");
        std::fs::write(
            &path,
            format!(
                "schema_version: prompt-registry.v1\nregistry_id: prompt-registry\n\
                 d127_import_eligible: false\nentries:\n{entries}"
            ),
        )
        .unwrap();
        path
    }

    const TASK_ENTRY: &str = "  - {prompt_id: article_summary, prompt_version: summary.v1, kind: task, schema_ref: article-summary.v1}\n";

    #[test]
    fn real_data_registry_resolves_both_kinds_with_computed_fingerprints() {
        let root = repo_root();
        let registry = load_prompt_registry(
            &root.join("data/ontology/prompt_registry.yaml"),
            &real_task_registry(),
            &root,
        )
        .unwrap();
        assert_eq!(registry.entries.len(), 4);
        let task = registry.resolve("article_summary", "summary.v1").unwrap();
        assert_eq!(task.kind, PromptEntryKind::Task);
        assert_eq!(
            task.source_ref,
            "data/ontology/prompt_tasks.yaml#article_summary"
        );
        let meta = registry.resolve("critic", "1.0.0").unwrap();
        assert_eq!(meta.kind, PromptEntryKind::MetaPrompt);
        assert_eq!(meta.prompt_sha256.as_str().len(), 64);
        assert_eq!(meta.schema_sha256.as_str().len(), 64);
    }

    #[test]
    fn unknown_field_is_refused() {
        let dir = scratch("unknown-field");
        let path = registry_file(&dir, TASK_ENTRY);
        let mut body = std::fs::read_to_string(&path).unwrap();
        body.push_str("unknown: true\n");
        std::fs::write(&path, body).unwrap();
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("parse prompt registry"), "{error}");
    }

    #[test]
    fn duplicate_identity_and_d127_unlock_are_refused() {
        let dir = scratch("duplicate");
        let path = registry_file(&dir, &format!("{TASK_ENTRY}{TASK_ENTRY}"));
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("duplicate"), "{error}");

        let dir = scratch("d127-unlock");
        let path = dir.join("prompt_registry.yaml");
        std::fs::write(
            &path,
            format!(
                "schema_version: prompt-registry.v1\nregistry_id: prompt-registry\n\
                 d127_import_eligible: true\nentries:\n{TASK_ENTRY}"
            ),
        )
        .unwrap();
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("d127"), "{error}");
    }

    #[test]
    fn unresolvable_task_reference_is_refused() {
        let dir = scratch("unknown-task");
        let path = registry_file(
            &dir,
            "  - {prompt_id: no_such_prompt, prompt_version: v1, kind: task, schema_ref: article-summary.v1}\n",
        );
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("no_such_prompt"), "{error}");

        let dir = scratch("wrong-task-version");
        let path = registry_file(
            &dir,
            "  - {prompt_id: article_summary, prompt_version: v9, kind: task, schema_ref: article-summary.v1}\n",
        );
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("summary.v1"), "{error}");
    }

    #[test]
    fn meta_prompt_requires_source_ref() {
        let dir = scratch("meta-missing-source");
        let path = registry_file(
            &dir,
            "  - {prompt_id: critic, prompt_version: \"1.0.0\", kind: meta_prompt, schema_ref: data/schemas/agent.schema.yaml}\n",
        );
        let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
        assert!(error.contains("requires source_ref"), "{error}");
    }

    #[test]
    fn meta_prompt_references_must_stay_inside_the_root() {
        for (name, source_ref) in [
            ("meta-parent-escape", "../outside.md"),
            ("meta-absolute", "/etc/hostname"),
        ] {
            let dir = scratch(name);
            let path = registry_file(
                &dir,
                &format!(
                    "  - {{prompt_id: critic, prompt_version: \"1.0.0\", kind: meta_prompt, source_ref: {source_ref}, schema_ref: data/schemas/agent.schema.yaml}}\n"
                ),
            );
            let error = load_prompt_registry(&path, &real_task_registry(), &dir).unwrap_err();
            assert!(error.contains("repo-relative path"), "{error}");
        }
    }

    #[test]
    fn meta_prompt_version_must_match_the_bound_agent_manifest() {
        let dir = scratch("meta-version-drift");
        let path = registry_file(
            &dir,
            "  - {prompt_id: critic, prompt_version: \"9.9.9\", kind: meta_prompt, source_ref: data/prompts/critic/system.v1.md, schema_ref: data/schemas/agent.schema.yaml}\n",
        );
        let error = load_prompt_registry(&path, &real_task_registry(), &repo_root()).unwrap_err();
        assert!(error.contains("agent_version"), "{error}");
    }
}
