//! Universal article acquisition & load plane — domain types (ADR-056 / D220).
//!
//! Pure identity, policy, stage, and durable LoadReport shapes.
//! No filesystem, network, or adapter types. Locator normalization is
//! provider-aware but not provider-rooted: arXiv is one family among many.
//!
//! Non-article units (textbook chapters) use the same report schema with
//! `source_profile` / `container_ref` metadata only — no graph edges here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema id stamped on every durable load report.
pub const LOAD_REPORT_SCHEMA_VERSION: &str = "load-report.v1";

// ─── Locator ────────────────────────────────────────────────────────────────

/// How the operator addressed a load target (raw input preserved separately).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArticleLocator {
    /// Versionless arXiv id (modern `YYMM.NNNNN` or legacy `cat/NNNNNNN`).
    Arxiv { id: String },
    /// Offline catalog pin: `catalog:<source_code>/.../<unit_key>`.
    Catalog {
        source_code: String,
        catalog_path: String,
    },
    /// Operator filesystem path (primary by extension later).
    Local { path: String },
    /// Unclassified raw locator kept for resolve by acquisition ports.
    Raw { value: String },
}

impl ArticleLocator {
    /// Parse and normalize a user locator string.
    ///
    /// Empty / whitespace-only → error. Prefer structured kinds when patterns
    /// match; otherwise `Raw` so providers can still `can_resolve`.
    pub fn parse(raw: &str) -> Result<Self, LoadDomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(LoadDomainError::EmptyLocator);
        }

        if let Some(rest) = strip_prefix_ci(trimmed, "catalog:") {
            return parse_catalog_pin(rest);
        }

        // Prefer arXiv normalize before local-path heuristics: URLs ending in
        // `.pdf` must not be misclassified as Local (ADR-056 locator honesty).
        if let Some(id) = try_normalize_arxiv_id(trimmed) {
            return Ok(Self::Arxiv { id });
        }

        if looks_like_local_path(trimmed) && !looks_like_url(trimmed) {
            return Ok(Self::Local {
                path: trimmed.to_string(),
            });
        }

        Ok(Self::Raw {
            value: trimmed.to_string(),
        })
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Arxiv { .. } => "arxiv",
            Self::Catalog { .. } => "catalog",
            Self::Local { .. } => "local",
            Self::Raw { .. } => "raw",
        }
    }

    /// Stable id-like key for reports (not always a paper id).
    pub fn canonical_key(&self) -> &str {
        match self {
            Self::Arxiv { id } => id.as_str(),
            Self::Catalog { catalog_path, .. } => catalog_path.as_str(),
            Self::Local { path } => path.as_str(),
            Self::Raw { value } => value.as_str(),
        }
    }
}

fn parse_catalog_pin(rest: &str) -> Result<ArticleLocator, LoadDomainError> {
    let path = rest.trim().trim_start_matches('/');
    if path.is_empty() {
        return Err(LoadDomainError::InvalidCatalogPin(
            "catalog: path is empty".into(),
        ));
    }
    let source_code = path
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LoadDomainError::InvalidCatalogPin("missing source_code".into()))?
        .to_string();
    // Catalog pin is structural; unknown source_code is validated by application
    // against SourceRegistry (ADR-056 fail-closed at ensure), not at parse.
    Ok(ArticleLocator::Catalog {
        source_code,
        catalog_path: path.to_string(),
    })
}

fn looks_like_url(s: &str) -> bool {
    strip_prefix_ci(s, "https://").is_some() || strip_prefix_ci(s, "http://").is_some()
}

fn looks_like_local_path(s: &str) -> bool {
    s.starts_with('/')
        || s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with("~/")
        || (s.len() > 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic())
        || s.ends_with(".pdf")
        || s.ends_with(".html")
        || s.ends_with(".htm")
        || s.ends_with(".md")
}

/// Normalize arXiv abs/pdf/html URLs and bare ids to versionless modern/legacy id.
/// Returns `None` if the string is not an arXiv locator.
pub fn try_normalize_arxiv_id(raw: &str) -> Option<String> {
    let mut candidate = raw.trim().to_string();
    if candidate.is_empty() {
        return None;
    }

    // arxiv: prefix
    if let Some(rest) = strip_prefix_ci(&candidate, "arxiv:") {
        candidate = rest.trim().to_string();
    }

    // URLs
    for prefix in [
        "https://export.arxiv.org/abs/",
        "http://export.arxiv.org/abs/",
        "https://export.arxiv.org/pdf/",
        "http://export.arxiv.org/pdf/",
        "https://export.arxiv.org/html/",
        "http://export.arxiv.org/html/",
        "https://arxiv.org/abs/",
        "http://arxiv.org/abs/",
        "https://arxiv.org/pdf/",
        "http://arxiv.org/pdf/",
        "https://arxiv.org/html/",
        "http://arxiv.org/html/",
    ] {
        if let Some(rest) = strip_prefix_ci(&candidate, prefix) {
            candidate = rest.to_string();
            break;
        }
    }

    if let Some(rest) = strip_suffix_ci(&candidate, ".pdf") {
        candidate = rest.to_string();
    }

    // Drop trailing path segments for modern ids only (legacy has slash).
    if let Some((head, _tail)) = candidate.split_once('/')
        && head.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        candidate = head.to_string();
    }

    // Strip version suffix vN
    if let Some(pos) = candidate.rfind('v') {
        let (base, ver) = candidate.split_at(pos);
        if ver.len() > 1 && ver[1..].chars().all(|c| c.is_ascii_digit()) && !base.is_empty() {
            candidate = base.to_string();
        }
    }

    candidate = candidate.trim().trim_end_matches('/').to_string();

    if is_modern_arxiv_id(&candidate) || is_legacy_arxiv_id(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

fn is_modern_arxiv_id(s: &str) -> bool {
    // YYMM.NNNN or YYMM.NNNNN
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return false;
    }
    let (a, b) = (parts[0], parts[1]);
    a.len() == 4
        && a.chars().all(|c| c.is_ascii_digit())
        && (b.len() == 4 || b.len() == 5)
        && b.chars().all(|c| c.is_ascii_digit())
}

fn is_legacy_arxiv_id(s: &str) -> bool {
    // e.g. hep-th/9901001
    let Some((cat, num)) = s.split_once('/') else {
        return false;
    };
    if cat.is_empty() || !cat.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        return false;
    }
    if !cat
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    {
        return false;
    }
    num.len() == 7 && num.chars().all(|c| c.is_ascii_digit())
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    if s.len() >= suffix.len() && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix) {
        Some(&s[..s.len() - suffix.len()])
    } else {
        None
    }
}

// ─── Policy & stages ────────────────────────────────────────────────────────

/// Acquisition / load policy object (ADR-056 §2.4 D) — not scattered flags only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionPolicy {
    /// Network is fail-closed by default.
    pub allow_network: bool,
    pub include_html: bool,
    pub include_tex_source: bool,
    pub dry_run: bool,
    /// D127: default locked.
    pub enable_import: bool,
    pub stages: LoadStageSet,
    pub run_mode: LoadRunMode,
}

impl Default for AcquisitionPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            include_html: true,
            include_tex_source: false,
            dry_run: false,
            enable_import: false,
            stages: LoadStageSet::EnsureIngest,
            run_mode: LoadRunMode::Inline,
        }
    }
}

impl AcquisitionPolicy {
    pub fn ensure_only() -> Self {
        Self {
            stages: LoadStageSet::Ensure,
            ..Self::default()
        }
    }

    pub fn import_eligible_stamp(&self) -> bool {
        self.enable_import
    }
}

/// Which load stages to run (ADR-056 §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadStageSet {
    Ensure,
    EnsureIngest,
    EnsureIngestExtract,
    Full,
}

impl LoadStageSet {
    pub fn includes_ingest(self) -> bool {
        !matches!(self, Self::Ensure)
    }

    pub fn includes_extract(self) -> bool {
        matches!(self, Self::EnsureIngestExtract | Self::Full)
    }

    pub fn includes_export_snapshot(self) -> bool {
        matches!(
            self,
            Self::Full | Self::EnsureIngest | Self::EnsureIngestExtract
        )
    }
}

/// Individual stage names for per-stage status on the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadStageName {
    Resolve,
    EnsureAssets,
    Ingest,
    Extract,
    ExportSnapshot,
    EmitLoadReport,
}

/// Bulk/retry execution mode — same use case (ADR-056 §2.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadRunMode {
    Inline,
    Enqueue,
    Worker,
}

/// Disposition of an ensured asset or stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetDisposition {
    Ok,
    Unchanged,
    CatalogHit,
    Fetched,
    Skip,
    Fail,
    NotRequested,
    NotLicensed,
}

/// Primary content kind selected for the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryContentKind {
    Pdf,
    Html,
    Markdown,
    Latex,
    Unknown,
}

// ─── LoadReport ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadReport {
    pub schema_version: String,
    pub locator_raw: String,
    pub locator: ArticleLocator,
    pub provider: String,
    pub source_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_or_topic: Option<String>,
    /// Non-article container (book title/id) — string metadata only (ADR-056 §2.11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_kind: Option<PrimaryContentKind>,
    pub graph_readiness: String,
    pub stages: Vec<StageStatus>,
    pub assets: Vec<LoadAssetRecord>,
    #[serde(default)]
    pub cache: Vec<LoadCacheRecord>,
    pub queue: LoadQueueRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_sha256: Option<String>,
    pub import_eligible: bool,
    pub ok: bool,
    pub duration_ms: u64,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub findings: Vec<LoadFinding>,
    /// Content digest over canonical report body (excluding this field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_digest_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageStatus {
    pub stage: LoadStageName,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadAssetRecord {
    pub kind: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    pub disposition: AssetDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadCacheRecord {
    pub stage_id: String,
    /// hit | miss | bypass | retryable_failure
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadQueueRecord {
    pub run_mode: LoadRunMode,
    #[serde(default)]
    pub tasks_opened: Vec<LoadQueueTaskRef>,
    #[serde(default)]
    pub tasks_completed: Vec<LoadQueueTaskRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadQueueTaskRef {
    pub task_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadFinding {
    pub code: String,
    pub message: String,
}

impl LoadReport {
    pub fn new_skeleton(locator_raw: impl Into<String>, locator: ArticleLocator) -> Self {
        Self {
            schema_version: LOAD_REPORT_SCHEMA_VERSION.to_string(),
            locator_raw: locator_raw.into(),
            locator,
            provider: String::new(),
            source_code: String::new(),
            source_profile: None,
            category_or_topic: None,
            container_ref: None,
            unit_key: None,
            primary_kind: None,
            graph_readiness: "not_claimed".to_string(),
            stages: Vec::new(),
            assets: Vec::new(),
            cache: Vec::new(),
            queue: LoadQueueRecord {
                run_mode: LoadRunMode::Inline,
                tasks_opened: Vec::new(),
                tasks_completed: Vec::new(),
            },
            snapshot_path: None,
            snapshot_sha256: None,
            import_eligible: false,
            ok: false,
            duration_ms: 0,
            errors: Vec::new(),
            findings: Vec::new(),
            report_digest_sha256: None,
        }
    }

    /// SHA-256 over canonical JSON with `report_digest_sha256` cleared.
    pub fn compute_digest(&self) -> String {
        let mut for_hash = self.clone();
        for_hash.report_digest_sha256 = None;
        let json = serde_json::to_vec(&for_hash).expect("LoadReport serializes");
        let mut hasher = Sha256::new();
        hasher.update(&json);
        hex::encode(hasher.finalize())
    }

    pub fn seal_digest(&mut self) {
        self.report_digest_sha256 = Some(self.compute_digest());
    }
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadDomainError {
    EmptyLocator,
    InvalidCatalogPin(String),
    InvalidArxivId(String),
}

impl fmt::Display for LoadDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLocator => write!(f, "empty locator"),
            Self::InvalidCatalogPin(m) => write!(f, "invalid catalog pin: {m}"),
            Self::InvalidArxivId(m) => write!(f, "invalid arxiv id: {m}"),
        }
    }
}

impl std::error::Error for LoadDomainError {}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_locator_fails() {
        assert!(matches!(
            ArticleLocator::parse(""),
            Err(LoadDomainError::EmptyLocator)
        ));
        assert!(matches!(
            ArticleLocator::parse("   "),
            Err(LoadDomainError::EmptyLocator)
        ));
    }

    #[test]
    fn normalize_arxiv_bare_and_url() {
        let cases = [
            ("2602.04837", "2602.04837"),
            ("arxiv:2602.04837", "2602.04837"),
            ("https://arxiv.org/abs/2602.04837", "2602.04837"),
            ("https://arxiv.org/pdf/2602.04837.pdf", "2602.04837"),
            ("https://arxiv.org/html/2602.04837v1", "2602.04837"),
            ("https://export.arxiv.org/pdf/2602.04837v2", "2602.04837"),
            ("2507.19457v3", "2507.19457"),
        ];
        for (raw, want) in cases {
            let loc = ArticleLocator::parse(raw).expect(raw);
            match loc {
                ArticleLocator::Arxiv { id } => assert_eq!(id, want, "raw={raw}"),
                other => panic!("expected Arxiv for {raw}, got {other:?}"),
            }
        }
    }

    #[test]
    fn normalize_legacy_arxiv() {
        let loc = ArticleLocator::parse("hep-th/9901001").unwrap();
        match loc {
            ArticleLocator::Arxiv { id } => assert_eq!(id, "hep-th/9901001"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn catalog_pin_parses_source_and_path() {
        let raw = "catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs";
        let loc = ArticleLocator::parse(raw).unwrap();
        match loc {
            ArticleLocator::Catalog {
                source_code,
                catalog_path,
            } => {
                assert_eq!(source_code, "gnn_textbook");
                assert_eq!(
                    catalog_path,
                    "gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs"
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(crate::is_known_source_code("gnn_textbook"));
    }

    #[test]
    fn catalog_pin_empty_fails() {
        assert!(matches!(
            ArticleLocator::parse("catalog:"),
            Err(LoadDomainError::InvalidCatalogPin(_))
        ));
    }

    #[test]
    fn local_path_detected() {
        for raw in ["./papers/x.pdf", "/tmp/a.html", "notes.md"] {
            let loc = ArticleLocator::parse(raw).unwrap();
            assert!(
                matches!(loc, ArticleLocator::Local { .. }),
                "raw={raw} -> {loc:?}"
            );
        }
    }

    #[test]
    fn unknown_string_stays_raw() {
        let loc = ArticleLocator::parse("doi:10.1234/foo").unwrap();
        match loc {
            ArticleLocator::Raw { value } => assert_eq!(value, "doi:10.1234/foo"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn policy_defaults_fail_closed() {
        let p = AcquisitionPolicy::default();
        assert!(!p.allow_network);
        assert!(!p.enable_import);
        assert!(!p.dry_run);
        assert!(!p.import_eligible_stamp());
        assert!(p.stages.includes_ingest());
        assert!(!p.stages.includes_extract());
        assert_eq!(p.run_mode, LoadRunMode::Inline);

        let e = AcquisitionPolicy::ensure_only();
        assert!(!e.stages.includes_ingest());
        assert!(!e.stages.includes_extract());
    }

    #[test]
    fn load_report_serde_roundtrip_and_digest_stable() {
        let locator = ArticleLocator::parse("2602.04837").unwrap();
        let mut report = LoadReport::new_skeleton("2602.04837", locator);
        report.provider = "arxiv".into();
        report.source_code = "arxiv".into();
        report.source_profile = Some("scientific_paper".into());
        report.primary_kind = Some(PrimaryContentKind::Pdf);
        report.stages.push(StageStatus {
            stage: LoadStageName::Resolve,
            status: "ok".into(),
            duration_ms: Some(1),
            detail: None,
        });
        report.stages.push(StageStatus {
            stage: LoadStageName::EnsureAssets,
            status: "catalog_hit".into(),
            duration_ms: Some(2),
            detail: Some("pdf present".into()),
        });
        report.assets.push(LoadAssetRecord {
            kind: "pdf".into(),
            path:
                "data/article_catalog/article_catalog/arxiv/cs-ai/2602.04837/source/2602.04837.pdf"
                    .into(),
            sha256: Some("abc".into()),
            bytes: Some(12),
            disposition: AssetDisposition::CatalogHit,
        });
        report.cache.push(LoadCacheRecord {
            stage_id: "parse".into(),
            status: "miss".into(),
            key_sha256: Some("k".into()),
            payload_sha256: None,
        });
        report.queue.run_mode = LoadRunMode::Inline;
        report.import_eligible = false;
        report.ok = true;
        report.duration_ms = 3;
        report.seal_digest();

        assert_eq!(report.schema_version, LOAD_REPORT_SCHEMA_VERSION);
        assert!(!report.import_eligible);
        assert_eq!(report.graph_readiness, "not_claimed");

        crate::roundtrip_json(&report);

        let d1 = report.report_digest_sha256.clone().unwrap();
        let d2 = report.compute_digest();
        assert_eq!(d1, d2, "sealed digest must match recompute");
        // Second seal must not change body digest identity when field stripped
        report.seal_digest();
        assert_eq!(report.report_digest_sha256.as_deref(), Some(d1.as_str()));
    }

    #[test]
    fn non_article_report_carries_container_not_edges() {
        let raw = "catalog:gnn_textbook/html/gnn-ch-about";
        let locator = ArticleLocator::parse(raw).unwrap();
        let mut report = LoadReport::new_skeleton(raw, locator);
        report.source_code = "gnn_textbook".into();
        report.source_profile = Some("textbook".into());
        report.container_ref = Some("Graph Neural Networks Textbook".into());
        report.unit_key = Some("gnn-ch-about".into());
        report.primary_kind = Some(PrimaryContentKind::Html);
        report.graph_readiness = "not_claimed".into();
        report.ok = true;
        report.seal_digest();
        crate::roundtrip_json(&report);
        // No graph edge fields exist on LoadReport — container_ref is metadata only.
        assert!(report.container_ref.is_some());
        assert_eq!(report.graph_readiness, "not_claimed");
    }

    #[test]
    fn try_normalize_rejects_non_arxiv() {
        assert!(try_normalize_arxiv_id("not-an-id").is_none());
        assert!(try_normalize_arxiv_id("10.1234/xyz").is_none());
    }
}
