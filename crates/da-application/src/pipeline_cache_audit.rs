//! Application audit joining cache manifests to durable dual lineage.

use da_domain::{assurance::ContentDigest, pipeline_cache_lineage::CacheLineageEvidence};
use da_ports::pipeline_cache::{StageCacheKey, StageManifestEntry, StageOutcome};

pub fn audit_cache_success(
    key: &StageCacheKey,
    manifest: &StageManifestEntry,
    cache_hit: bool,
    graph_materialization_skipped: bool,
) -> Result<CacheLineageEvidence, String> {
    let expected = key.key_sha256().map_err(|e| e.to_string())?;
    if manifest.key != *key || manifest.key_sha256 != expected {
        return Err("cache manifest effective-input identity mismatch".into());
    }
    let (payload_sha256, payload_size) = match &manifest.outcome {
        StageOutcome::Success {
            payload_sha256,
            payload_size,
        } => (payload_sha256, *payload_size),
        StageOutcome::RetryableFailure { .. } => {
            return Err("retryable failure has no successful payload lineage".into());
        }
    };
    let dependencies = key
        .dependency_output_sha256
        .iter()
        .map(|(stage, digest)| {
            ContentDigest::try_from(digest.clone())
                .map(|value| (stage.clone(), value))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let evidence = CacheLineageEvidence {
        article_id: key.source.article_id.clone(),
        stage_id: key.stage_id.clone(),
        key_sha256: ContentDigest::try_from(expected).map_err(|e| e.to_string())?,
        payload_sha256: ContentDigest::try_from(payload_sha256.clone())
            .map_err(|e| e.to_string())?,
        payload_size,
        dependency_output_sha256: dependencies,
        implementation_fingerprint: key.implementation_fingerprint.clone(),
        configuration_fingerprint: key.configuration_fingerprint.clone(),
        prompt_fingerprint: key.prompt_fingerprint.clone(),
        model_fingerprint: key.model_fingerprint.clone(),
        output_schema_fingerprint: key.output_schema_fingerprint.clone(),
        cache_hit,
        graph_materialization_skipped,
        import_eligible: false,
    };
    evidence.validate()?;
    Ok(evidence)
}
