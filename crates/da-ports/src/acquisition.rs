//! Source acquisition port — resolve locators and ensure catalog assets (ADR-056 / D220).
//!
//! Application depends on this port only. Adapters implement provider-specific
//! fetch/catalog layout. Network is fail-closed via `AcquisitionPolicy`.

use da_domain::{
    AcquisitionPolicy, ArticleLocator, AssetDisposition, LoadDomainError, PrimaryContentKind,
};
use thiserror::Error;

/// Driven port: provider-agnostic acquisition (ADR-056 §2.3).
pub trait SourceAcquisitionPort: Send + Sync {
    /// Registered `source_code` from DATA (`data/source_codes.yaml`).
    fn provider_code(&self) -> &str;

    fn can_resolve(&self, locator: &ArticleLocator) -> bool;

    fn resolve(&self, locator: &ArticleLocator) -> Result<ResolvedArticleRef, AcquisitionError>;

    fn ensure_assets(
        &self,
        resolved: &ResolvedArticleRef,
        policy: &AcquisitionPolicy,
    ) -> Result<EnsuredAssets, AcquisitionError>;
}

/// Stable identity after resolve (before/after ensure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArticleRef {
    pub source_code: String,
    pub unit_key: String,
    /// Repo-relative catalog root for the unit when known (e.g. arxiv/cs-ai/id).
    pub catalog_path: Option<String>,
    pub locator: ArticleLocator,
    pub raw_locator: String,
    /// Genre for process (scientific_paper | textbook | …) — report metadata only.
    pub source_profile: Option<String>,
    /// Non-article container title/id (book) — string metadata only (ADR-056/057).
    pub container_ref: Option<String>,
}

/// Result of ensure_assets — primary selection is explicit (never "first file").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsuredAssets {
    pub primary: EnsuredPrimary,
    pub assets: Vec<EnsuredAsset>,
    /// True when no network was used and catalog already had primary.
    pub catalog_hit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsuredPrimary {
    pub kind: PrimaryContentKind,
    /// Repo-relative path preferred when under catalog.
    pub path: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsuredAsset {
    pub kind: String,
    pub path: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub disposition: AssetDisposition,
}

#[derive(Debug, Error)]
pub enum AcquisitionError {
    #[error("locator domain error: {0}")]
    Domain(#[from] LoadDomainError),
    #[error("cannot resolve locator with provider {provider}: {detail}")]
    CannotResolve { provider: String, detail: String },
    #[error("network denied (--disallow-network): {0}")]
    NetworkDenied(String),
    #[error("asset missing: {0}")]
    MissingAsset(String),
    #[error("checksum conflict: {0}")]
    ChecksumConflict(String),
    #[error("unknown source_code (not in registry): {0}")]
    UnknownSourceCode(String),
    #[error("acquisition infrastructure: {0}")]
    Infrastructure(String),
}

// ─── Mock (ADR-051 port contract contour) ───────────────────────────────────

/// In-memory acquisition for contract tests. No FS/network.
#[derive(Debug, Clone)]
pub struct MockSourceAcquisition {
    pub provider: String,
    /// When set, ensure returns catalog_hit primary without network.
    pub hit: Option<EnsuredAssets>,
    /// When true and no hit, ensure requires allow_network (then still fails unless hit).
    pub require_network_if_miss: bool,
    pub source_profile: Option<String>,
    pub container_ref: Option<String>,
}

impl MockSourceAcquisition {
    pub fn catalog_hit_pdf(_unit_key: &str, path: &str, sha: &str) -> Self {
        Self {
            provider: "arxiv".into(),
            hit: Some(EnsuredAssets {
                primary: EnsuredPrimary {
                    kind: PrimaryContentKind::Pdf,
                    path: path.into(),
                    sha256: Some(sha.into()),
                    bytes: Some(100),
                },
                assets: vec![EnsuredAsset {
                    kind: "pdf".into(),
                    path: path.into(),
                    sha256: Some(sha.into()),
                    bytes: Some(100),
                    disposition: AssetDisposition::CatalogHit,
                }],
                catalog_hit: true,
            }),
            require_network_if_miss: true,
            source_profile: Some("scientific_paper".into()),
            container_ref: None,
        }
    }

    pub fn miss_requires_network() -> Self {
        Self {
            provider: "arxiv".into(),
            hit: None,
            require_network_if_miss: true,
            source_profile: Some("scientific_paper".into()),
            container_ref: None,
        }
    }

    /// Textbook-shaped mock: HTML primary + container metadata (ADR-057 report only).
    pub fn catalog_hit_html_textbook(unit_key: &str, path: &str, container: &str) -> Self {
        Self {
            provider: "gnn_textbook".into(),
            hit: Some(EnsuredAssets {
                primary: EnsuredPrimary {
                    kind: PrimaryContentKind::Html,
                    path: path.into(),
                    sha256: Some("htmlsha".into()),
                    bytes: Some(50),
                },
                assets: vec![EnsuredAsset {
                    kind: "html".into(),
                    path: path.into(),
                    sha256: Some("htmlsha".into()),
                    bytes: Some(50),
                    disposition: AssetDisposition::CatalogHit,
                }],
                catalog_hit: true,
            }),
            require_network_if_miss: true,
            source_profile: Some("textbook".into()),
            container_ref: Some(container.into()),
        }
        .with_unit_key_hint(unit_key)
    }

    fn with_unit_key_hint(self, _unit_key: &str) -> Self {
        self
    }
}

impl SourceAcquisitionPort for MockSourceAcquisition {
    fn provider_code(&self) -> &str {
        &self.provider
    }

    fn can_resolve(&self, locator: &ArticleLocator) -> bool {
        match locator {
            ArticleLocator::Arxiv { .. } => self.provider == "arxiv",
            ArticleLocator::Catalog { source_code, .. } => source_code == &self.provider,
            ArticleLocator::Local { .. } => self.provider == "local",
            ArticleLocator::Raw { .. } => false,
        }
    }

    fn resolve(&self, locator: &ArticleLocator) -> Result<ResolvedArticleRef, AcquisitionError> {
        if !self.can_resolve(locator) {
            return Err(AcquisitionError::CannotResolve {
                provider: self.provider.clone(),
                detail: format!("locator kind {}", locator.kind_name()),
            });
        }
        let unit_key = locator.canonical_key().to_string();
        let catalog_path = match locator {
            ArticleLocator::Catalog { catalog_path, .. } => Some(catalog_path.clone()),
            _ => None,
        };
        Ok(ResolvedArticleRef {
            source_code: self.provider.clone(),
            unit_key,
            catalog_path,
            locator: locator.clone(),
            raw_locator: locator.canonical_key().to_string(),
            source_profile: self.source_profile.clone().or_else(|| {
                if self.provider == "arxiv" {
                    Some("scientific_paper".into())
                } else {
                    None
                }
            }),
            container_ref: self.container_ref.clone(),
        })
    }

    fn ensure_assets(
        &self,
        resolved: &ResolvedArticleRef,
        policy: &AcquisitionPolicy,
    ) -> Result<EnsuredAssets, AcquisitionError> {
        if let Some(hit) = &self.hit {
            return Ok(hit.clone());
        }
        if self.require_network_if_miss && !policy.allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "missing primary for {} and network denied",
                resolved.unit_key
            )));
        }
        if policy.dry_run {
            return Err(AcquisitionError::MissingAsset(format!(
                "dry_run: would fetch {} but no mock payload",
                resolved.unit_key
            )));
        }
        Err(AcquisitionError::MissingAsset(format!(
            "no mock assets for {}",
            resolved.unit_key
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;

    #[test]
    fn mock_catalog_hit_without_network() {
        let mock = MockSourceAcquisition::catalog_hit_pdf(
            "2602.04837",
            "data/sources/arxiv/cs-ai/2602.04837/source/2602.04837.pdf",
            "deadbeef",
        );
        let loc = ArticleLocator::parse("2602.04837").unwrap();
        assert!(mock.can_resolve(&loc));
        let resolved = mock.resolve(&loc).unwrap();
        assert_eq!(resolved.unit_key, "2602.04837");
        let policy = AcquisitionPolicy::ensure_only(); // allow_network=false
        let assets = mock.ensure_assets(&resolved, &policy).unwrap();
        assert!(assets.catalog_hit);
        assert_eq!(assets.primary.kind, PrimaryContentKind::Pdf);
        assert_eq!(assets.assets[0].disposition, AssetDisposition::CatalogHit);
    }

    #[test]
    fn mock_miss_fails_closed_without_network() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let loc = ArticleLocator::parse("9999.99999").unwrap();
        let resolved = mock.resolve(&loc).unwrap();
        let policy = AcquisitionPolicy::ensure_only();
        assert!(!policy.allow_network);
        let err = mock.ensure_assets(&resolved, &policy).unwrap_err();
        assert!(matches!(err, AcquisitionError::NetworkDenied(_)));
    }

    #[test]
    fn mock_miss_with_network_still_missing_without_payload() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let loc = ArticleLocator::parse("9999.99999").unwrap();
        let resolved = mock.resolve(&loc).unwrap();
        let policy = AcquisitionPolicy {
            allow_network: true,
            ..AcquisitionPolicy::default()
        };
        let err = mock.ensure_assets(&resolved, &policy).unwrap_err();
        assert!(matches!(err, AcquisitionError::MissingAsset(_)));
    }

    #[test]
    fn mock_rejects_unrelated_locator() {
        let mock = MockSourceAcquisition::miss_requires_network();
        let loc = ArticleLocator::parse("doi:10.1/x").unwrap();
        assert!(!mock.can_resolve(&loc));
        assert!(mock.resolve(&loc).is_err());
    }

    #[test]
    fn mock_catalog_pin_for_matching_provider() {
        let mut mock = MockSourceAcquisition::catalog_hit_pdf("u", "p", "s");
        mock.provider = "gnn_textbook".into();
        let loc = ArticleLocator::parse("catalog:gnn_textbook/html/gnn-ch-about").unwrap();
        assert!(mock.can_resolve(&loc));
        let r = mock.resolve(&loc).unwrap();
        assert_eq!(r.source_code, "gnn_textbook");
        assert!(r.catalog_path.as_deref().unwrap().contains("gnn-ch-about"));
    }
}
