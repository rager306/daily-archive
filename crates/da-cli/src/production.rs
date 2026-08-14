//! Shared infrastructure context for production article commands.
//!
//! This module owns environment, filesystem and concrete adapter defaults. It
//! intentionally does not execute stages or move infrastructure defaults into
//! da-application.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use da_adapters::{FilePipelineCache, SamyamaGraphStore};
use da_domain::{PipelineStageLedger, StageIdRegistry};

const PIPELINE_LEDGER_YAML: &str =
    include_str!("../../../data/ontology/pipeline_stage_contracts.yaml");

pub struct ProductionContext {
    pub store: Arc<SamyamaGraphStore>,
    pub cache: Option<Arc<FilePipelineCache>>,
    pub clock: Arc<dyn da_ports::Clock>,
    pub stage_registry: Arc<StageIdRegistry>,
}

impl ProductionContext {
    pub fn new(
        store: Arc<SamyamaGraphStore>,
        cache_root: Option<&Path>,
        materialization_time: Option<&str>,
    ) -> Result<Self, String> {
        let cache = match cache_root {
            Some(root) => {
                std::fs::create_dir_all(root).map_err(|error| {
                    format!("create pipeline cache {}: {error}", root.display())
                })?;
                Some(Arc::new(FilePipelineCache::new(root)))
            }
            None => None,
        };
        let clock: Arc<dyn da_ports::Clock> =
            match parse_materialization_time(materialization_time)? {
                None => Arc::new(da_ports::SystemClock),
                Some(parsed) => Arc::new(da_ports::FixedClock::from_timestamp(parsed.timestamp())),
            };
        let stage_registry = embedded_stage_registry()?;
        Ok(Self {
            store,
            cache,
            clock,
            stage_registry,
        })
    }

    pub fn from_env(
        cache_root: Option<&Path>,
        materialization_time: Option<&str>,
    ) -> Result<Self, String> {
        let store = Arc::new(
            SamyamaGraphStore::try_from_env()
                .map_err(|error| format!("graph store open failed: {error}"))?,
        );
        Self::new(store, cache_root, materialization_time)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRevision {
    context: da_application::RevisionContentContext,
    method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCatalogPdf {
    pub path: PathBuf,
    pub revision_context: Option<da_application::RevisionContentContext>,
    pub binding_method: &'static str,
}

/// Resolve a catalog PDF for a legacy command. If a bare PDF is byte-identical
/// to one authoritative versioned manifestation, return that exact revision;
/// otherwise return the bare path with an explicit versionless fallback.
pub fn resolve_catalog_pdf(
    catalog_root: &Path,
    paper_id: &str,
) -> Result<Option<ResolvedCatalogPdf>, String> {
    let mut bare = None;
    let mut versioned = Vec::new();
    for entry in walkdir::WalkDir::new(catalog_root)
        .max_depth(12)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".pdf") else {
            continue;
        };
        if stem == paper_id {
            bare = Some(path.to_path_buf());
        } else if let Some(revision) = stem
            .strip_prefix(format!("{paper_id}v").as_str())
            .and_then(|value| value.parse::<u32>().ok())
            && revision > 0
        {
            versioned.push(path.to_path_buf());
        }
    }
    versioned.sort();

    let bare_sha = match &bare {
        Some(path) => {
            let bytes =
                std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
            use sha2::Digest as _;
            Some(hex::encode(sha2::Sha256::digest(bytes)))
        }
        None => None,
    };

    let mut exact = Vec::new();
    for path in &versioned {
        match revision_context_for_versioned_pdf(path, paper_id) {
            Ok(Some(context)) => exact.push((
                path.clone(),
                ResolvedRevision {
                    context,
                    method: "versioned_filename_checksum",
                },
            )),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if let Some(bare_sha) = bare_sha
        && exact.len() > 1
    {
        let matching = exact
            .iter()
            .filter(|(_, binding)| {
                binding
                    .context
                    .manifestations
                    .iter()
                    .any(|manifestation| manifestation.sha256 == bare_sha)
            })
            .collect::<Vec<_>>();
        if matching.len() == 1 {
            let binding = &matching[0].1;
            return Ok(Some(ResolvedCatalogPdf {
                path: bare.expect("bare digest source"),
                revision_context: Some(binding.context.clone()),
                binding_method: binding.method,
            }));
        }
    }
    if exact.len() == 1 {
        let (path, binding) = exact.into_iter().next().expect("one exact binding");
        return Ok(Some(ResolvedCatalogPdf {
            path,
            revision_context: Some(binding.context),
            binding_method: binding.method,
        }));
    }
    if exact.len() > 1 {
        return Err(format!("ambiguous exact catalog revisions for {paper_id}"));
    }
    if let Some(path) = bare {
        return Ok(Some(ResolvedCatalogPdf {
            path,
            revision_context: None,
            binding_method: "versionless_fallback",
        }));
    }
    Ok(None)
}

pub fn parse_materialization_time(
    value: Option<&str>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    value
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map(|time| time.with_timezone(&chrono::Utc))
                .map_err(|error| format!("invalid --materialization-time {value:?}: {error}"))
        })
        .transpose()
}

pub fn embedded_stage_registry() -> Result<Arc<StageIdRegistry>, String> {
    let ledger: PipelineStageLedger = serde_yaml::from_str(PIPELINE_LEDGER_YAML)
        .map_err(|error| format!("parse embedded pipeline ledger: {error}"))?;
    Ok(Arc::new(ledger.registry()?))
}

/// Derive an exact revision context from a versioned local PDF filename.
/// Bare versionless input returns None; this helper never invents revision 1.
pub fn revision_context_for_versioned_pdf(
    pdf_path: &Path,
    paper_id: &str,
) -> Result<Option<da_application::RevisionContentContext>, String> {
    let Some(name) = pdf_path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    let Some(stem) = name.strip_suffix(".pdf") else {
        return Ok(None);
    };
    let Some(revision) = stem
        .strip_prefix(format!("{paper_id}v").as_str())
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return Ok(None);
    };
    if revision == 0 {
        return Err(format!("invalid revision 0 in {}", pdf_path.display()));
    }
    let bytes =
        std::fs::read(pdf_path).map_err(|error| format!("read {}: {error}", pdf_path.display()))?;
    if bytes.is_empty() {
        return Err(format!("empty versioned source {}", pdf_path.display()));
    }
    use sha2::Digest as _;
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    let manifestation = da_application::RevisionManifestationInput {
        format: "pdf".into(),
        sha256,
        bytes: bytes.len() as u64,
        uri: format!("catalog:{}", pdf_path.display()),
        path: pdf_path.display().to_string(),
        capture_status: "captured_local".into(),
    };
    let locator = da_domain::ArticleLocator::Arxiv {
        id: paper_id.to_string(),
        revision: Some(revision),
    };
    let classifier =
        da_application::RevisionClassifyUseCase::bundled().map_err(|error| error.to_string())?;
    classifier
        .revision_context_for_classified_load(&locator, "arxiv", paper_id, vec![manifestation])
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_loads_registry_cache_and_fixed_clock() {
        let root =
            std::env::temp_dir().join(format!("da-production-context-{}", std::process::id()));
        let context = ProductionContext::new(
            Arc::new(SamyamaGraphStore::new()),
            Some(&root),
            Some("2023-11-14T22:13:20Z"),
        )
        .unwrap();
        assert_eq!(
            context.stage_registry.resolve("embed").unwrap().as_str(),
            "embedding"
        );
        assert_eq!(
            context.clock.now(),
            chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap()
        );
        assert_eq!(context.cache.as_ref().unwrap().root(), root.as_path());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_resolver_prefers_exact_version_for_identical_bare_bytes() {
        let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/article_catalog");
        let resolved = resolve_catalog_pdf(&catalog, "2506.07853")
            .unwrap()
            .expect("catalog paper");
        let context = resolved.revision_context.expect("exact v5 context");
        assert_eq!(context.revision, 5);
        assert_eq!(resolved.binding_method, "versioned_filename_checksum");
        assert!(
            resolved
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "2506.07853.pdf")
                || resolved
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "2506.07853v5.pdf")
        );
    }

    #[test]
    fn production_command_stage_inventory_is_canonical() {
        let registry = embedded_stage_registry().unwrap();
        let canonical = ["parse", "embedding", "extraction_compute", "summary"];
        for stage in canonical {
            registry.parse_canonical(stage).unwrap();
        }
        assert_eq!(registry.resolve("embed").unwrap().as_str(), "embedding");
        assert_eq!(
            registry.resolve("extract").unwrap().as_str(),
            "extraction_compute"
        );
        assert!(registry.parse_canonical("semantic_enrichment").is_err());
    }

    #[test]
    fn versioned_pdf_adapter_binds_exact_and_never_invents_v1() {
        let root =
            std::env::temp_dir().join(format!("da-production-revision-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let versioned = root.join("2506.07853v5.pdf");
        std::fs::write(&versioned, b"version-five").unwrap();
        let context = revision_context_for_versioned_pdf(&versioned, "2506.07853")
            .unwrap()
            .expect("exact context");
        assert_eq!(context.revision, 5);
        assert_eq!(context.manifestations.len(), 1);
        assert_eq!(context.manifestations[0].bytes, 12);

        let bare = root.join("2506.07853.pdf");
        std::fs::write(&bare, b"version-five").unwrap();
        assert!(
            revision_context_for_versioned_pdf(&bare, "2506.07853")
                .unwrap()
                .is_none()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_materialization_time_fails_before_context_creation() {
        let error =
            ProductionContext::new(Arc::new(SamyamaGraphStore::new()), None, Some("not-a-time"))
                .err()
                .unwrap();
        assert!(error.contains("invalid --materialization-time"), "{error}");
    }
}
