//! Atomic parse artifact cache over [`FilePipelineCache`] (S06/T03).
//!
//! Implements the provider-neutral [`da_parse::ParseArtifactCache`] seam
//! (M464-z84in2 S06/T01) on top of the canonical atomic pipeline cache
//! (payload-before-manifest, checksum-verified reads):
//!
//! 1. **Two independent stage namespaces.** Raw verbatim TEI
//!    (`parse.raw_tei.v1`) and the `ParsedArticle` projection
//!    (`parse.project_article.v1`) live under separate stage directories and
//!    separate storage identities, so a projector fix or schema rotation
//!    never invalidates raw TEI (and vice versa).
//! 2. **Content-addressed keys, no new key material.** Stage keys arrive in
//!    the canonical `<stage_id>:<64 hex>` form minted by `da_parse`
//!    (endpoint + full dialect wire + source bytes for raw; paper id + source
//!    SHA + verified raw payload SHA + projector schema fingerprint for the
//!    projection). This adapter validates the key shape and translates it to
//!    the [`StageCacheKey`] storage identity — the logical source identity is
//!    deliberately derived from the key itself (`MetadataOnly`, hex shard),
//!    so no paths, env, roots or re-derivation logic enter this layer.
//! 3. **Corruption is an error, never a silent miss.** Checksum drift maps
//!    to [`CacheIntegrityCode::ChecksumMismatch`], a manifest that does not
//!    belong to the requested key to
//!    [`CacheIntegrityCode::IdentityMismatch`], any uninterpretable/corrupt
//!    manifest or IO state to [`CacheIntegrityCode::ManifestCorrupt`], and
//!    failed publication to [`CacheIntegrityCode::StageWriteFailed`]. A
//!    missing manifest stays a normal miss (the commit marker is absent).
//!    Cross-namespace key misuse (a raw key handed to the projection
//!    methods) is [`CacheIntegrityCode::IdentityMismatch`], not a miss.
//!
//! The payload bytes are opaque here: envelope encoding/verification
//! (`RawTeiEnvelopePayload` with verbatim TEI + exact backend/profile/result
//! identity; `ProjectArticlePayload` with `dependency_raw_payload_sha256`
//! bound to the raw payload) is owned and enforced by `da_parse`. The raw
//! binding therefore travels in the payload identity, not in storage config.
//!
//! The module also owns the extracted **legacy rich-PDF read fallback**
//! ([`lookup_with_legacy_parse_identity`]): the exact collapsed identity
//! `da load` used before the two-stage namespaces existed. Legacy reads are
//! read-only — a legacy hit never creates a canonical record.
//!
//! No tracing of payloads, paths, credentials or roots: integrity warnings
//! carry the stage and integrity code only.

use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use da_parse::GrobidProfile;
use da_parse::{
    CacheIntegrityCode, PROJECT_ARTICLE_STAGE_ID, ParseArtifactCache, ParseCacheFailure,
    ParseCacheStage, RAW_TEI_STAGE_ID, RawStageRead,
};
use da_ports::pipeline_cache::{
    CacheLookup, PipelineCache, PipelineCacheError, SourceFormat, SourceIdentity, StageCacheKey,
};
use tracing::debug;

use crate::pipeline_cache::FilePipelineCache;

/// Storage-layer implementation identity of this adapter (part of the
/// storage `StageCacheKey`, never of the semantic `da_parse` stage keys).
const ARTIFACT_IMPLEMENTATION_FINGERPRINT: &str = "da_adapters::FileParseArtifactCache@v1";

/// Constant configuration fingerprint: stage keys are already
/// content-addressed by `da_parse`, so this layer adds no configuration
/// axis (config-incompatible parse profiles simply occupy different keys).
const ARTIFACT_CONFIGURATION_FINGERPRINT: &str = "content-addressed-v1";

/// Constant source-normalization fingerprint of content-addressed artifact
/// keys (the source normalization already happened before key minting).
const ARTIFACT_SOURCE_FINGERPRINT: &str = "parse-artifact-key/v1";

/// Storage-layer payload schema pin of the raw stage: the
/// `RawTeiEnvelopePayload` JSON envelope (schema-versioned separately by
/// `RAW_TEI_PAYLOAD_SCHEMA_VERSION`).
const RAW_STAGE_STORAGE_SCHEMA: &str = "RawTeiEnvelopePayload@serde-v1";

/// Storage-layer payload schema pin of the projection stage: the
/// `ProjectArticlePayload` JSON envelope.
const PROJECTED_STAGE_STORAGE_SCHEMA: &str = "ProjectArticlePayload@serde-v1";

/// Hex shard length of the canonical stage-key form (`<stage_id>:<64 hex>`).
const STAGE_KEY_HEX_LEN: usize = 64;

/// The stage identity of the OTHER parse-artifact namespace (used to
/// distinguish cross-namespace key misuse from structurally malformed keys).
fn other_namespace(stage: ParseCacheStage) -> &'static str {
    match stage {
        ParseCacheStage::RawTei => PROJECT_ARTICLE_STAGE_ID,
        ParseCacheStage::ProjectArticle => RAW_TEI_STAGE_ID,
    }
}

/// Validate the canonical stage-key form and return its hex shard.
///
/// - a key naming the OTHER namespace is cross-namespace misuse →
///   [`CacheIntegrityCode::IdentityMismatch`] on the addressed stage;
/// - any structurally malformed key (wrong shape, non-canonical hex) →
///   [`CacheIntegrityCode::ManifestCorrupt`] on the addressed stage.
fn parse_stage_key_hex(stage: ParseCacheStage, key: &str) -> Result<String, ParseCacheFailure> {
    let prefix = format!("{}:", stage.stage_id());
    let hex = match key.strip_prefix(&prefix) {
        Some(hex) => hex,
        None if key.starts_with(other_namespace(stage)) => {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::IdentityMismatch,
            ));
        }
        None => {
            return Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::ManifestCorrupt,
            ));
        }
    };
    let canonical_hex = hex.len() == STAGE_KEY_HEX_LEN
        && hex
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    if !canonical_hex {
        return Err(ParseCacheFailure::new(
            stage,
            CacheIntegrityCode::ManifestCorrupt,
        ));
    }
    Ok(hex.to_owned())
}

/// Map a read-side [`PipelineCacheError`] onto the closed integrity
/// vocabulary. Identity mismatches stay distinguishable from generic
/// corruption via the marker message owned by `pipeline_cache.rs`.
fn read_integrity_failure(stage: ParseCacheStage, error: PipelineCacheError) -> ParseCacheFailure {
    let failure = match error {
        PipelineCacheError::ChecksumMismatch { .. } => {
            ParseCacheFailure::new(stage, CacheIntegrityCode::ChecksumMismatch)
        }
        PipelineCacheError::InvalidManifest { message, .. }
            if message.contains(crate::pipeline_cache::MANIFEST_KEY_MISMATCH_MESSAGE) =>
        {
            ParseCacheFailure::new(stage, CacheIntegrityCode::IdentityMismatch)
        }
        // Io, Contract, unreadable/corrupt manifest JSON and any other
        // uninterpretable state: corruption, never a silent miss.
        _ => ParseCacheFailure::new(stage, CacheIntegrityCode::ManifestCorrupt),
    };
    debug!(
        stage = %failure.stage(),
        code = %failure.code(),
        "parse artifact cache read integrity failure"
    );
    failure
}

/// Atomic filesystem implementation of [`ParseArtifactCache`] over
/// [`FilePipelineCache`]. Both stage namespaces share one physical root;
/// each stage keeps its own directory subtree and storage identity.
#[derive(Debug, Clone)]
pub struct FileParseArtifactCache {
    cache: FilePipelineCache,
}

impl FileParseArtifactCache {
    /// Create the artifact cache over a fresh [`FilePipelineCache`] rooted
    /// at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::from_pipeline_cache(FilePipelineCache::new(root))
    }

    /// Create the artifact cache over an existing pipeline cache (shared
    /// root composition: the legacy lookup helpers address the same root).
    pub fn from_pipeline_cache(cache: FilePipelineCache) -> Self {
        Self { cache }
    }

    /// The underlying pipeline cache (same physical root; used by the
    /// legacy read helper and diagnostics).
    pub fn pipeline_cache(&self) -> &FilePipelineCache {
        &self.cache
    }

    /// Translate a canonical `da_parse` stage key into its storage
    /// [`StageCacheKey`]. Exposed for diagnostics and contract tests; no
    /// paths or payload identity leak through it.
    pub fn stage_cache_key(
        &self,
        stage: ParseCacheStage,
        key: &str,
    ) -> Result<StageCacheKey, ParseCacheFailure> {
        let hex_shard = parse_stage_key_hex(stage, key)?;
        Ok(StageCacheKey {
            source: SourceIdentity {
                // The hex shard is the content address: a safe, stable,
                // per-key directory shard derived from the key itself.
                article_id: hex_shard,
                selected_format: SourceFormat::MetadataOnly,
                pdf_sha256: None,
                html_sha256: None,
                acquisition_fingerprint: ARTIFACT_SOURCE_FINGERPRINT.into(),
                normalization_fingerprint: ARTIFACT_SOURCE_FINGERPRINT.into(),
            },
            stage_id: stage.stage_id().to_owned(),
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: ARTIFACT_IMPLEMENTATION_FINGERPRINT.into(),
            configuration_fingerprint: ARTIFACT_CONFIGURATION_FINGERPRINT.into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: match stage {
                ParseCacheStage::RawTei => RAW_STAGE_STORAGE_SCHEMA,
                ParseCacheStage::ProjectArticle => PROJECTED_STAGE_STORAGE_SCHEMA,
            }
            .into(),
        })
    }

    async fn read_stage(
        &self,
        stage: ParseCacheStage,
        key: &str,
    ) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        let storage_key = self.stage_cache_key(stage, key)?;
        let now = Utc::now().to_rfc3339();
        match PipelineCache::lookup(&self.cache, &storage_key, &now) {
            Ok(CacheLookup::Miss) => Ok(None),
            Ok(CacheLookup::Hit(hit)) => Ok(Some(RawStageRead::canonical(hit.payload))),
            // The artifact namespaces only ever hold success payloads; a
            // retryable-failure manifest here is schema-foreign content.
            Ok(CacheLookup::RetryableFailure(_)) => Err(ParseCacheFailure::new(
                stage,
                CacheIntegrityCode::ManifestCorrupt,
            )),
            Err(error) => Err(read_integrity_failure(stage, error)),
        }
    }

    async fn write_stage(
        &self,
        stage: ParseCacheStage,
        key: &str,
        payload: &[u8],
    ) -> Result<(), ParseCacheFailure> {
        let storage_key = self.stage_cache_key(stage, key)?;
        let now = Utc::now().to_rfc3339();
        PipelineCache::store_success(&self.cache, &storage_key, payload, BTreeMap::new(), &now)
            .map(|_| ())
            .map_err(|_| ParseCacheFailure::new(stage, CacheIntegrityCode::StageWriteFailed))
    }
}

#[async_trait]
impl ParseArtifactCache for FileParseArtifactCache {
    async fn read_raw(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.read_stage(ParseCacheStage::RawTei, key).await
    }

    async fn write_raw(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.write_stage(ParseCacheStage::RawTei, key, payload)
            .await
    }

    async fn read_projected(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.read_stage(ParseCacheStage::ProjectArticle, key).await
    }

    async fn write_projected(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.write_stage(ParseCacheStage::ProjectArticle, key, payload)
            .await
    }
}

// ---------------------------------------------------------------------------
// Legacy rich-PDF read fallback (extracted verbatim from `da load`)
// ---------------------------------------------------------------------------

/// One legacy identity parse lookup: the cache outcome plus the legacy
/// identity it was resolved from (`"pdf-grobid"` or `"html-primary"`),
/// exactly as `da load` reports it. Legacy hits are read-only: they never
/// create canonical records.
#[derive(Debug, Clone)]
pub struct LegacyParseIdentityLookup {
    /// The cache outcome under the legacy collapsed identity.
    pub lookup: CacheLookup,
    /// The legacy configuration identity the entry was resolved from, if
    /// any (`None` on a miss or when legacy reuse is not applicable).
    pub resolved_from: Option<String>,
}

/// The selected manifestation SHA of a source identity (the legacy collapsed
/// identity bound the whole acquisition fingerprint to the selected bytes).
fn selected_source_sha(source: &SourceIdentity) -> Option<String> {
    match source.selected_format {
        SourceFormat::Pdf => source.pdf_sha256.clone(),
        SourceFormat::Html => source.html_sha256.clone(),
        SourceFormat::MetadataOnly => None,
    }
}

/// Read the exact previous production parse key only for the semantics that
/// form actually had: rich PDF-only or HTML-only collapsed source identity.
/// Structural/custom profiles and dual-manifestation identities never reuse
/// it, and a legacy hit never creates a canonical record (lookup only).
///
/// `canonical_key` is the current production parse key
/// (`da_adapters::GrobidParser@load-v1` / `da_adapters::HtmlParser@load-v1`
/// with schema `ParsedArticle.v1`); the legacy lookup collapses the source
/// identity to the selected manifestation and swaps the configuration
/// fingerprint to the legacy stage identity (`"pdf-grobid"` /
/// `"html-primary"`).
pub fn lookup_with_legacy_parse_identity(
    cache: &FilePipelineCache,
    canonical_key: &StageCacheKey,
    is_html: bool,
    profile: GrobidProfile,
    now: &str,
) -> Result<LegacyParseIdentityLookup, String> {
    let legacy_configuration = if is_html {
        if canonical_key.source.pdf_sha256.is_some() {
            return Ok(missed());
        }
        "html-primary"
    } else {
        if profile != GrobidProfile::rich() || canonical_key.source.html_sha256.is_some() {
            return Ok(missed());
        }
        "pdf-grobid"
    };
    let Some(selected_sha) = selected_source_sha(&canonical_key.source) else {
        return Ok(missed());
    };
    let mut legacy_key = canonical_key.clone();
    legacy_key.source.pdf_sha256 = if is_html {
        None
    } else {
        Some(selected_sha.clone())
    };
    legacy_key.source.html_sha256 = if is_html {
        Some(selected_sha.clone())
    } else {
        None
    };
    legacy_key.source.acquisition_fingerprint = selected_sha;
    legacy_key.configuration_fingerprint = legacy_configuration.to_owned();
    let lookup = cache
        .lookup(&legacy_key, now)
        .map_err(|error| format!("legacy {legacy_configuration} parse cache lookup: {error}"))?;
    if matches!(lookup, CacheLookup::Miss) {
        Ok(missed())
    } else {
        Ok(LegacyParseIdentityLookup {
            lookup,
            resolved_from: Some(legacy_configuration.to_owned()),
        })
    }
}

fn missed() -> LegacyParseIdentityLookup {
    LegacyParseIdentityLookup {
        lookup: CacheLookup::Miss,
        resolved_from: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_parse::{PROJECT_ARTICLE_STAGE_ID, RAW_TEI_STAGE_ID};

    fn legacy_source(pdf_sha: Option<String>, html_sha: Option<String>) -> SourceIdentity {
        SourceIdentity {
            article_id: "test.paper".into(),
            selected_format: if pdf_sha.is_some() {
                SourceFormat::Pdf
            } else {
                SourceFormat::Html
            },
            pdf_sha256: pdf_sha,
            html_sha256: html_sha,
            acquisition_fingerprint: "catalog-v1".into(),
            normalization_fingerprint: "normalize-v1".into(),
        }
    }

    fn rich_pdf_only_key() -> StageCacheKey {
        StageCacheKey {
            source: legacy_source(Some("a".repeat(64)), None),
            stage_id: "parse".into(),
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: "da_adapters::GrobidParser@load-v1".into(),
            configuration_fingerprint: "profile=rich".into(),
            prompt_fingerprint: None,
            model_fingerprint: None,
            output_schema_fingerprint: "ParsedArticle.v1".into(),
        }
    }

    fn legacy_pdf_only_key(canonical: &StageCacheKey) -> StageCacheKey {
        let sha = canonical.source.pdf_sha256.clone().unwrap();
        let mut legacy = canonical.clone();
        legacy.source.acquisition_fingerprint = sha;
        legacy.configuration_fingerprint = "pdf-grobid".into();
        legacy
    }

    #[test]
    fn legacy_hit_serves_rich_pdf_only_without_canonical_write() {
        let root = tempfile::tempdir().unwrap();
        let cache = FilePipelineCache::new(root.path());
        let canonical = rich_pdf_only_key();
        let stored = cache
            .store_success(
                &legacy_pdf_only_key(&canonical),
                b"legacy-parsed-article",
                BTreeMap::new(),
                "2026-08-14T00:00:00Z",
            )
            .unwrap();
        let outcome = lookup_with_legacy_parse_identity(
            &cache,
            &canonical,
            false,
            GrobidProfile::rich(),
            "2026-08-14T00:01:00Z",
        )
        .unwrap();
        let CacheLookup::Hit(hit) = outcome.lookup else {
            panic!("expected legacy hit")
        };
        assert_eq!(hit.payload, b"legacy-parsed-article");
        assert_eq!(hit.manifest.key_sha256, stored.key_sha256);
        assert_eq!(outcome.resolved_from.as_deref(), Some("pdf-grobid"));
        // Read-only: the lookup must not have created any additional record.
        assert_eq!(cache.manifest("test.paper").unwrap().len(), 1);
    }

    #[test]
    fn legacy_read_is_denied_beyond_rich_pdf_only() {
        let root = tempfile::tempdir().unwrap();
        let cache = FilePipelineCache::new(root.path());
        let now = "2026-08-14T00:01:00Z";

        // Structural dialect: denied even with a stored entry.
        let mut structural = rich_pdf_only_key();
        structural.configuration_fingerprint = "profile=structural".into();
        cache
            .store_success(
                &legacy_pdf_only_key(&structural),
                b"legacy",
                BTreeMap::new(),
                now,
            )
            .unwrap();
        let outcome = lookup_with_legacy_parse_identity(
            &cache,
            &structural,
            false,
            GrobidProfile::structural(),
            now,
        )
        .unwrap();
        assert_eq!(outcome.lookup, CacheLookup::Miss);
        assert_eq!(outcome.resolved_from, None);

        // Dual PDF+HTML manifestation: denied for the rich profile too.
        let mut dual = rich_pdf_only_key();
        dual.source.html_sha256 = Some("b".repeat(64));
        let outcome =
            lookup_with_legacy_parse_identity(&cache, &dual, false, GrobidProfile::rich(), now)
                .unwrap();
        assert_eq!(outcome.lookup, CacheLookup::Miss);
        assert_eq!(outcome.resolved_from, None);

        // HTML primary with a PDF manifestation present: denied.
        let mut html_dual = rich_pdf_only_key();
        html_dual.source.selected_format = SourceFormat::Html;
        html_dual.source.pdf_sha256 = None;
        html_dual.source.html_sha256 = Some("b".repeat(64));
        let outcome =
            lookup_with_legacy_parse_identity(&cache, &html_dual, true, GrobidProfile::rich(), now)
                .unwrap();
        assert_eq!(outcome.lookup, CacheLookup::Miss);
        assert_eq!(outcome.resolved_from, None);
    }

    #[test]
    fn legacy_lookup_never_serves_a_versionless_entry_for_a_versioned_key() {
        // Regression for the version-vs-legacy hazard: the legacy lookup
        // clones the canonical key, so the probed `model_fingerprint` must
        // carry through and prevent a versioned run from reusing another
        // release's versionless legacy entry.
        let root = tempfile::tempdir().unwrap();
        let cache = FilePipelineCache::new(root.path());
        let canonical = rich_pdf_only_key();
        cache
            .store_success(
                &legacy_pdf_only_key(&canonical),
                b"legacy-parsed-article",
                BTreeMap::new(),
                "2026-08-14T00:00:00Z",
            )
            .unwrap();

        let mut versioned = canonical.clone();
        versioned.model_fingerprint = Some("grobid/grobid:0.9.1".into());
        let outcome = lookup_with_legacy_parse_identity(
            &cache,
            &versioned,
            false,
            GrobidProfile::rich(),
            "2026-08-14T00:01:00Z",
        )
        .unwrap();
        assert_eq!(
            outcome.lookup,
            CacheLookup::Miss,
            "a versioned run must not reuse a versionless legacy entry"
        );
        assert_eq!(outcome.resolved_from, None);

        // The explicit versionless key still serves the legacy entry.
        let legacy_ok = lookup_with_legacy_parse_identity(
            &cache,
            &canonical,
            false,
            GrobidProfile::rich(),
            "2026-08-14T00:01:00Z",
        )
        .unwrap();
        assert!(matches!(legacy_ok.lookup, CacheLookup::Hit(_)));
    }

    #[tokio::test]
    async fn stage_key_translation_rejects_foreign_and_malformed_keys() {
        let dir = tempfile::tempdir().unwrap();
        let cache = FileParseArtifactCache::new(dir.path());
        let raw = format!("{}:{}", RAW_TEI_STAGE_ID, "a".repeat(STAGE_KEY_HEX_LEN));
        let projected = format!(
            "{}:{}",
            PROJECT_ARTICLE_STAGE_ID,
            "b".repeat(STAGE_KEY_HEX_LEN)
        );

        // Cross-namespace misuse is an identity error on the addressed stage.
        let error = cache
            .stage_cache_key(ParseCacheStage::RawTei, &projected)
            .unwrap_err();
        assert_eq!(error.stage(), ParseCacheStage::RawTei);
        assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);
        let error = cache
            .stage_cache_key(ParseCacheStage::ProjectArticle, &raw)
            .unwrap_err();
        assert_eq!(error.stage(), ParseCacheStage::ProjectArticle);
        assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);

        // Malformed keys are corruption, never a miss.
        for malformed in [
            "parse.raw_tei.v1",
            "parse.raw_tei.v1:zz",
            &format!("{}:ABC", RAW_TEI_STAGE_ID),
            "garbage",
        ] {
            let error = cache
                .stage_cache_key(ParseCacheStage::RawTei, malformed)
                .unwrap_err();
            assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);
        }

        // Canonical keys translate with the hex shard as the article pin.
        let storage = cache
            .stage_cache_key(ParseCacheStage::RawTei, &raw)
            .unwrap();
        assert_eq!(storage.stage_id, RAW_TEI_STAGE_ID);
        assert_eq!(storage.source.article_id, "a".repeat(STAGE_KEY_HEX_LEN));
    }
}
