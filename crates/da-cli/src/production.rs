//! Shared infrastructure context for production article commands.
//!
//! This module owns environment, filesystem and concrete adapter defaults. It
//! intentionally does not execute stages or move infrastructure defaults into
//! da-application.

use std::path::Path;
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
    fn invalid_materialization_time_fails_before_context_creation() {
        let error =
            ProductionContext::new(Arc::new(SamyamaGraphStore::new()), None, Some("not-a-time"))
                .err()
                .unwrap();
        assert!(error.contains("invalid --materialization-time"), "{error}");
    }
}
