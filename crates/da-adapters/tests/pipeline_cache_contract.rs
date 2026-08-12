use std::{
    collections::BTreeMap,
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use da_adapters::{memory_pipeline_cache::MemoryPipelineCache, pipeline_cache::FilePipelineCache};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, SourceFormat, SourceIdentity, StageCacheKey,
};

fn key(config: &str) -> StageCacheKey {
    StageCacheKey {
        source: SourceIdentity {
            article_id: "2507.19457".into(),
            selected_format: SourceFormat::Pdf,
            pdf_sha256: Some("a".repeat(64)),
            html_sha256: Some("b".repeat(64)),
            acquisition_fingerprint: "catalog-v1".into(),
            normalization_fingerprint: "normalize-v1".into(),
        },
        stage_id: "parse".into(),
        dependency_output_sha256: BTreeMap::from([("catalog".into(), "c".repeat(64))]),
        implementation_fingerprint: "parser-v1".into(),
        configuration_fingerprint: config.into(),
        prompt_fingerprint: None,
        model_fingerprint: Some("grobid-v1".into()),
        output_schema_fingerprint: "ParsedArticle-v1".into(),
    }
}
fn contract(cache: Arc<dyn PipelineCache>) {
    let k = key("config-v1");
    assert!(matches!(
        cache.lookup(&k, "2026-08-12T00:00:00Z").unwrap(),
        CacheLookup::Miss
    ));
    let entry = cache
        .store_success(
            &k,
            b"payload",
            BTreeMap::from([("sections".into(), "2".into())]),
            "2026-08-12T00:00:00Z",
        )
        .unwrap();
    assert_eq!(entry.key_sha256, k.key_sha256().unwrap());
    match cache.lookup(&k, "2026-08-12T00:00:01Z").unwrap() {
        CacheLookup::Hit(hit) => {
            assert_eq!(hit.payload, b"payload");
            assert_eq!(hit.manifest.key_sha256, k.key_sha256().unwrap());
        }
        other => panic!("expected hit, got {other:?}"),
    };
    let drift = key("config-v2");
    assert_ne!(k.key_sha256().unwrap(), drift.key_sha256().unwrap());
    assert!(matches!(
        cache.lookup(&drift, "2026-08-12T00:00:01Z").unwrap(),
        CacheLookup::Miss
    ));
    assert_eq!(cache.manifest("2507.19457").unwrap().len(), 1);
    cache
        .store_retryable_failure(
            &drift,
            "rate_limit",
            "bounded",
            "2026-08-12T00:02:00Z",
            "2026-08-12T00:01:00Z",
        )
        .unwrap();
    assert!(matches!(
        cache.lookup(&drift, "2026-08-12T00:01:30Z").unwrap(),
        CacheLookup::RetryableFailure(_)
    ));
    assert!(matches!(
        cache.lookup(&drift, "2026-08-12T00:02:01Z").unwrap(),
        CacheLookup::Miss
    ));
}
#[test]
fn memory_cache_passes_shared_contract() {
    contract(Arc::new(MemoryPipelineCache::new()));
}
#[test]
fn filesystem_cache_passes_shared_contract() {
    let root = temp("shared");
    contract(Arc::new(FilePipelineCache::new(&root)));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn concurrent_same_key_publication_remains_a_verified_hit() {
    let root = temp("concurrent");
    let cache = Arc::new(FilePipelineCache::new(&root));
    let k = key("same");
    let mut joins = vec![];
    for _ in 0..8 {
        let c = cache.clone();
        let k = k.clone();
        joins.push(thread::spawn(move || {
            c.store_success(&k, b"same-payload", BTreeMap::new(), "2026-08-12T00:00:00Z")
        }));
    }
    for j in joins {
        j.join().unwrap().unwrap();
    }
    assert!(matches!(
        cache.lookup(&k, "2026-08-12T00:00:01Z").unwrap(),
        CacheLookup::Hit(_)
    ));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn filesystem_corruption_is_visible_not_a_miss() {
    let root = temp("corrupt");
    let cache = FilePipelineCache::new(&root);
    let k = key("corrupt");
    cache
        .store_success(&k, b"original", BTreeMap::new(), "2026-08-12T00:00:00Z")
        .unwrap();
    let payload = root
        .join("2507.19457/parse")
        .join(k.key_sha256().unwrap())
        .join("payload.bin");
    std::fs::write(payload, b"corrupt").unwrap();
    assert!(matches!(
        cache.lookup(&k, "2026-08-12T00:00:01Z"),
        Err(PipelineCacheError::ChecksumMismatch { .. })
    ));
    std::fs::remove_dir_all(root).unwrap();
}
fn temp(name: &str) -> std::path::PathBuf {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("da-cache-contract-{name}-{n}"))
}
