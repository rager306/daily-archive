use std::{
    collections::BTreeMap,
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use da_adapters::{
    memory_pipeline_cache::MemoryPipelineCache,
    parse_cache::{FileParseArtifactCache, lookup_with_legacy_parse_identity},
    pipeline_cache::FilePipelineCache,
};
use da_parse::{
    CacheIntegrityCode, FulltextDialectV1, GrobidProfile, ParseArtifactCache, ParseBackendRequest,
    ParseCacheStage, ProjectArticlePayload, RAW_TEI_STAGE_ID, RawTeiEnvelopePayload, TeiEnvelope,
    TeiIdentity, TeiResultIdentity, project_article_stage_key, raw_tei_stage_key,
};
use da_ports::parser::ParsedArticle;
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, SourceFormat, SourceIdentity, StageCacheKey,
};
use sha2::Digest;

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

// ── S06/T03: FileParseArtifactCache — two namespaces over one atomic root ──

fn raw_stage_key() -> String {
    raw_tei_stage_key(&ParseBackendRequest::new("paper-id", b"pdf-bytes".to_vec()))
}

fn projected_stage_key(raw_payload_sha: &str) -> String {
    project_article_stage_key(
        "paper-id",
        &"a".repeat(64),
        raw_payload_sha,
        "ParsedArticle@serde-v3-unique-navigation",
    )
}

#[tokio::test]
async fn parse_artifact_cache_serves_both_namespaces_independently() {
    let root = temp("artifact-namespaces");
    let cache = FileParseArtifactCache::new(&root);

    let raw = raw_stage_key();
    let raw_payload = b"<tei>verbatim</tei>".to_vec();
    cache.write_raw(&raw, &raw_payload).await.unwrap();

    let raw_payload_sha = hex::encode(sha2::Sha256::digest(&raw_payload));
    let projected = projected_stage_key(&raw_payload_sha);
    let article_payload = br#"{"paper_id":"paper-id"}"#.to_vec();
    cache
        .write_projected(&projected, &article_payload)
        .await
        .unwrap();

    let raw_read = cache.read_raw(&raw).await.unwrap().expect("raw hit");
    assert_eq!(raw_read.payload(), b"<tei>verbatim</tei>");
    assert!(!raw_read.is_legacy());
    let projected_read = cache
        .read_projected(&projected)
        .await
        .unwrap()
        .expect("projected hit");
    assert_eq!(projected_read.payload(), article_payload.as_slice());

    // Rotation independence: replacing the projection leaves raw verbatim.
    cache
        .write_projected(&projected, br#"{"paper_id":"paper-id","v2":true}"#)
        .await
        .unwrap();
    assert_eq!(
        cache.read_raw(&raw).await.unwrap().unwrap().payload(),
        b"<tei>verbatim</tei>"
    );

    // Unknown canonical keys are plain misses on both stages.
    assert!(
        cache
            .read_raw(&format!("{RAW_TEI_STAGE_ID}:{}", "c".repeat(64)))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        cache
            .read_projected(&projected_stage_key(&"d".repeat(64)))
            .await
            .unwrap()
            .is_none()
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn parse_artifact_cache_cross_namespace_and_malformed_keys_are_errors() {
    let root = temp("artifact-keys");
    let cache = FileParseArtifactCache::new(&root);
    let raw = raw_stage_key();
    let projected = projected_stage_key(&"b".repeat(64));

    let error = cache.read_raw(&projected).await.unwrap_err();
    assert_eq!(error.stage(), ParseCacheStage::RawTei);
    assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);
    let error = cache.read_projected(&raw).await.unwrap_err();
    assert_eq!(error.stage(), ParseCacheStage::ProjectArticle);
    assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);

    for malformed in ["parse.raw_tei.v1:zz", "parse.raw_tei.v1", "garbage"] {
        let error = cache.read_raw(malformed).await.unwrap_err();
        assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);
    }

    // Writes validate identically and never publish a rejected key.
    cache.write_projected(&raw, b"nope").await.unwrap_err();
    assert!(cache.read_projected(&projected).await.unwrap().is_none());

    // Nothing was ever written, so the temp root may not exist at all.
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn parse_artifact_cache_corruption_and_identity_drift_fail_visible() {
    let root = temp("artifact-corrupt");
    let cache = FileParseArtifactCache::new(&root);
    let raw = raw_stage_key();
    cache.write_raw(&raw, b"verbatim-tei").await.unwrap();

    let storage = cache
        .stage_cache_key(ParseCacheStage::RawTei, &raw)
        .unwrap();
    let entry = root
        .join(&storage.source.article_id)
        .join(&storage.stage_id)
        .join(storage.key_sha256().unwrap());
    // Checksum drift is a typed error, never a miss.
    std::fs::write(entry.join("payload.bin"), b"corrupted").unwrap();
    let error = cache.read_raw(&raw).await.unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ChecksumMismatch);

    // A manifest transplanted from another entry fails identity visibly.
    let other = format!("{RAW_TEI_STAGE_ID}:{}", "e".repeat(64));
    cache.write_raw(&other, b"other-entry").await.unwrap();
    let other_storage = cache
        .stage_cache_key(ParseCacheStage::RawTei, &other)
        .unwrap();
    let other_entry = root
        .join(&other_storage.source.article_id)
        .join(&other_storage.stage_id)
        .join(other_storage.key_sha256().unwrap());
    std::fs::copy(
        entry.join("manifest.json"),
        other_entry.join("manifest.json"),
    )
    .unwrap();
    let error = cache.read_raw(&other).await.unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);

    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn parse_artifact_cache_payload_without_manifest_is_a_miss() {
    let root = temp("artifact-uncommitted");
    let cache = FileParseArtifactCache::new(&root);
    let raw = raw_stage_key();
    let storage = cache
        .stage_cache_key(ParseCacheStage::RawTei, &raw)
        .unwrap();
    let entry = root
        .join(&storage.source.article_id)
        .join(&storage.stage_id)
        .join(storage.key_sha256().unwrap());
    std::fs::create_dir_all(&entry).unwrap();
    std::fs::write(entry.join("payload.bin"), b"uncommitted").unwrap();
    // No manifest.json: the commit marker is absent — a plain miss.
    assert!(cache.read_raw(&raw).await.unwrap().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_parse_artifact_publication_is_atomic_and_deterministic() {
    let root = temp("artifact-concurrent");
    let cache = Arc::new(FileParseArtifactCache::new(&root));
    let raw = raw_stage_key();
    let mut joins = Vec::new();
    for _ in 0..8 {
        let cache = cache.clone();
        let raw = raw.clone();
        joins.push(tokio::spawn(async move {
            cache.write_raw(&raw, b"same-verbatim-tei").await
        }));
    }
    for join in joins {
        join.await.unwrap().unwrap();
    }
    let read = cache.read_raw(&raw).await.unwrap().expect("published");
    assert_eq!(read.payload(), b"same-verbatim-tei");
    std::fs::remove_dir_all(root).unwrap();
}

// ── S06/T03: full versioned payload round-trips + legacy ParsedArticle reads ──

/// The complete staged payload — the versioned `RawTeiEnvelopePayload` and
/// `ProjectArticlePayload` envelopes over a real `ParsedArticle` — written
/// through the artifact cache and read back byte-exact, then re-verified by
/// `da_parse`'s own `decode_and_verify` (S06/T03 verify proof 1).
#[tokio::test]
async fn parse_artifact_cache_roundtrips_full_versioned_payloads() {
    let root = temp("artifact-full-payloads");
    let cache = FileParseArtifactCache::new(&root);

    let tei = "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\">full staged payload</TEI>";
    let result = TeiResultIdentity::fulltext(FulltextDialectV1::legacy_rich());
    let envelope = TeiEnvelope::new(
        tei,
        TeiIdentity::new("identity-gate-backend", "profile-fp").with_result(result.clone()),
    );
    let raw_payload = RawTeiEnvelopePayload::build(&envelope, &result);
    let raw = raw_stage_key();
    cache.write_raw(&raw, &raw_payload.encode()).await.unwrap();

    let raw_read = cache.read_raw(&raw).await.unwrap().expect("raw staged hit");
    let (verified_envelope, raw_sha) =
        RawTeiEnvelopePayload::decode_and_verify(raw_read.payload(), &result)
            .expect("raw payload survives the cache byte-exact");
    assert_eq!(verified_envelope.raw_tei(), tei.as_bytes());
    assert_eq!(raw_sha, raw_payload.payload_sha256());

    let article = ParsedArticle {
        paper_id: "paper-id".into(),
        title: "Atomic Payload Cache".into(),
        abstract_text: "round trip".into(),
        body_text: "projected body".into(),
        sections: vec![],
        citations: vec![],
        layout_json: None,
        tei_xml: Some(tei.to_string()),
        pdf_hash: "a".repeat(64),
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
    };
    let projector_key = projected_stage_key(&raw_sha);
    let projected_payload = ProjectArticlePayload::build(
        &article,
        "ParsedArticle@serde-v3-unique-navigation",
        &raw_sha,
        "paper-id",
        &"a".repeat(64),
    );
    cache
        .write_projected(&projector_key, &projected_payload.encode())
        .await
        .unwrap();

    let projected_read = cache
        .read_projected(&projector_key)
        .await
        .unwrap()
        .expect("projected staged hit");
    let decoded = ProjectArticlePayload::decode_and_verify(
        projected_read.payload(),
        "ParsedArticle@serde-v3-unique-navigation",
        &raw_sha,
        "paper-id",
        &"a".repeat(64),
    )
    .expect("projected payload survives the cache byte-exact");
    assert_eq!(
        serde_json::to_string(&decoded).unwrap(),
        serde_json::to_string(&article).unwrap()
    );

    std::fs::remove_dir_all(root).unwrap();
}

/// A REAL legacy `ParsedArticle` record — the exact pre-S06 `da load` write
/// form (collapsed `pdf-grobid` identity, `ParsedArticle.v1` JSON without
/// the newer defaulted fields) — reads back byte-identical through the
/// extracted fallback, decodes with today's `ParsedArticle` schema, and
/// stays read-only (S06/T03 verify proof 2; no migration, no canonical
/// record, no rewrite).
#[test]
fn legacy_parsed_article_reads_back_byte_identical_and_read_only() {
    let root = temp("artifact-legacy-parsed");
    let cache = FilePipelineCache::new(&root);

    let pdf_sha = "a".repeat(64);
    let canonical_source = SourceIdentity {
        article_id: "test.paper".into(),
        selected_format: SourceFormat::Pdf,
        pdf_sha256: Some(pdf_sha.clone()),
        html_sha256: None,
        acquisition_fingerprint: format!("acq-v2;pdf={pdf_sha};html=none"),
        normalization_fingerprint: "identity-v1".into(),
    };
    let canonical_key = StageCacheKey {
        source: canonical_source.clone(),
        stage_id: "parse".into(),
        dependency_output_sha256: BTreeMap::new(),
        implementation_fingerprint: "da_adapters::GrobidParser@load-v1".into(),
        configuration_fingerprint: GrobidProfile::rich().configuration_fingerprint(),
        prompt_fingerprint: None,
        model_fingerprint: None,
        output_schema_fingerprint: "ParsedArticle.v1".into(),
    };
    // The collapsed legacy identity exactly as the pre-S06 writer stored it:
    // acquisition pinned to the selected PDF bytes, configuration
    // "pdf-grobid", everything else byte-identical to the canonical key.
    let stored_legacy_key = StageCacheKey {
        source: SourceIdentity {
            acquisition_fingerprint: pdf_sha.clone(),
            ..canonical_source
        },
        configuration_fingerprint: "pdf-grobid".into(),
        ..canonical_key.clone()
    };

    // Pre-S06 JSON: only the nine original fields — the newer defaulted
    // fields (tables/formulas/missingness) did not exist when it was written.
    let legacy_payload = serde_json::json!({
        "paper_id": "test.paper",
        "title": "Legacy stored article",
        "abstract_text": "",
        "body_text": "legacy body",
        "sections": [],
        "citations": [],
        "layout_json": null,
        "tei_xml": null,
        "pdf_hash": pdf_sha,
    })
    .to_string()
    .into_bytes();
    cache
        .store_success(
            &stored_legacy_key,
            &legacy_payload,
            BTreeMap::new(),
            "2026-08-14T00:00:00Z",
        )
        .unwrap();

    let outcome = lookup_with_legacy_parse_identity(
        &cache,
        &canonical_key,
        false,
        GrobidProfile::rich(),
        "2026-08-14T00:01:00Z",
    )
    .expect("legacy lookup succeeds");
    let CacheLookup::Hit(hit) = outcome.lookup else {
        panic!("expected the stored legacy ParsedArticle to hit")
    };
    assert_eq!(outcome.resolved_from.as_deref(), Some("pdf-grobid"));
    assert_eq!(hit.payload, legacy_payload);
    let decoded: ParsedArticle = serde_json::from_slice(&hit.payload)
        .expect("legacy JSON still decodes with the current ParsedArticle schema");
    assert_eq!(decoded.paper_id, "test.paper");
    assert!(decoded.tables.is_empty());
    assert!(decoded.formulas.is_empty());

    // Read-only: the lookup created no canonical record (exactly one entry).
    assert_eq!(cache.manifest("test.paper").unwrap().len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
