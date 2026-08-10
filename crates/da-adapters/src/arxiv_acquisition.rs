//! ArXiv source acquisition adapter — offline catalog_hit first (ADR-056 W4).
//!
//! Network fetch is intentionally **not** implemented in this wave. Missing PDF
//! with `allow_network=false` → `NetworkDenied`. With `allow_network=true` still
//! returns a typed error until W10 Rust fetch / script boundary lands.
//!
//! Never call Python from `da-application`; any future script boundary stays here.

use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER: &str = "arxiv";

/// Filesystem-backed arXiv acquisition (catalog_hit path).
#[derive(Debug, Clone)]
pub struct ArxivSourceAcquisition {
    /// Root containing `article_catalog/...` or the `article_catalog` dir itself.
    pub catalog_root: PathBuf,
}

impl ArxivSourceAcquisition {
    pub fn new(catalog_root: impl Into<PathBuf>) -> Self {
        Self {
            catalog_root: catalog_root.into(),
        }
    }

    fn find_pdf(&self, arxiv_id: &str) -> Option<PathBuf> {
        let name = format!("{arxiv_id}.pdf");
        find_named_file(&self.catalog_root, &name)
    }
}

impl SourceAcquisitionPort for ArxivSourceAcquisition {
    fn provider_code(&self) -> &str {
        PROVIDER
    }

    fn can_resolve(&self, locator: &ArticleLocator) -> bool {
        matches!(locator, ArticleLocator::Arxiv { .. })
            || matches!(
                locator,
                ArticleLocator::Catalog { source_code, .. } if source_code == PROVIDER
            )
    }

    fn resolve(&self, locator: &ArticleLocator) -> Result<ResolvedArticleRef, AcquisitionError> {
        if !self.can_resolve(locator) {
            return Err(AcquisitionError::CannotResolve {
                provider: PROVIDER.into(),
                detail: format!("unsupported locator {}", locator.kind_name()),
            });
        }

        let (unit_key, catalog_path) = match locator {
            ArticleLocator::Arxiv { id } => (id.clone(), None),
            ArticleLocator::Catalog {
                catalog_path,
                source_code,
            } => {
                if source_code != PROVIDER {
                    return Err(AcquisitionError::CannotResolve {
                        provider: PROVIDER.into(),
                        detail: format!("catalog source_code={source_code}"),
                    });
                }
                let unit = catalog_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(catalog_path)
                    .to_string();
                (unit, Some(catalog_path.clone()))
            }
            _ => {
                return Err(AcquisitionError::CannotResolve {
                    provider: PROVIDER.into(),
                    detail: "not arxiv".into(),
                });
            }
        };

        Ok(ResolvedArticleRef {
            source_code: PROVIDER.into(),
            unit_key,
            catalog_path,
            locator: locator.clone(),
            raw_locator: locator.canonical_key().to_string(),
        })
    }

    fn ensure_assets(
        &self,
        resolved: &ResolvedArticleRef,
        policy: &AcquisitionPolicy,
    ) -> Result<EnsuredAssets, AcquisitionError> {
        if let Some(pdf) = self.find_pdf(&resolved.unit_key) {
            let meta = fs::metadata(&pdf).map_err(|e| {
                AcquisitionError::Infrastructure(format!("stat {}: {e}", pdf.display()))
            })?;
            let bytes = meta.len();
            let sha = sha256_file(&pdf).map_err(|e| {
                AcquisitionError::Infrastructure(format!("hash {}: {e}", pdf.display()))
            })?;
            let path_str = path_for_report(&self.catalog_root, &pdf);
            return Ok(EnsuredAssets {
                primary: EnsuredPrimary {
                    kind: PrimaryContentKind::Pdf,
                    path: path_str.clone(),
                    sha256: Some(sha.clone()),
                    bytes: Some(bytes),
                },
                assets: vec![EnsuredAsset {
                    kind: "pdf".into(),
                    path: path_str,
                    sha256: Some(sha),
                    bytes: Some(bytes),
                    disposition: AssetDisposition::CatalogHit,
                }],
                catalog_hit: true,
            });
        }

        if !policy.allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "PDF for {} not under {}; re-run with allow_network when fetch is wired",
                resolved.unit_key,
                self.catalog_root.display()
            )));
        }

        // W4: network path not implemented (script/native fetch later).
        Err(AcquisitionError::Infrastructure(format!(
            "network fetch for arXiv {} not implemented in W4 catalog_hit adapter",
            resolved.unit_key
        )))
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn path_for_report(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file.to_string_lossy().into_owned())
}

/// Depth-limited walk for a file name under catalog (avoids unbounded find).
fn find_named_file(root: &Path, file_name: &str) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    const MAX_NODES: usize = 50_000;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > MAX_NODES {
            break;
        }
        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == file_name)
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;
    use da_ports::SourceAcquisitionPort;
    use std::io::Write;

    fn write_fixture(root: &Path, arxiv_id: &str, pdf_bytes: &[u8]) -> PathBuf {
        let dir = root
            .join("article_catalog")
            .join("arxiv")
            .join("cs-ai")
            .join(arxiv_id)
            .join("source");
        fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join(format!("{arxiv_id}.pdf"));
        let mut f = fs::File::create(&pdf).unwrap();
        f.write_all(pdf_bytes).unwrap();
        pdf
    }

    #[test]
    fn catalog_hit_offline() {
        let root = std::env::temp_dir().join(format!("da-arxiv-acq-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_fixture(&root, "2602.04837", b"%PDF-1.4 fixture");

        let acq = ArxivSourceAcquisition::new(&root);
        let loc = ArticleLocator::parse("2602.04837").unwrap();
        assert!(acq.can_resolve(&loc));
        let resolved = acq.resolve(&loc).unwrap();
        let assets = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap();
        assert!(assets.catalog_hit);
        assert_eq!(assets.primary.kind, PrimaryContentKind::Pdf);
        assert!(assets.primary.sha256.is_some());
        assert_eq!(assets.assets[0].disposition, AssetDisposition::CatalogHit);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn miss_without_network_is_denied() {
        let root = std::env::temp_dir().join(format!("da-arxiv-acq-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let acq = ArxivSourceAcquisition::new(&root);
        let loc = ArticleLocator::parse("1111.22222").unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap_err();
        assert!(matches!(err, AcquisitionError::NetworkDenied(_)));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn network_true_still_not_implemented() {
        let root = std::env::temp_dir().join(format!("da-arxiv-acq-net-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let acq = ArxivSourceAcquisition::new(&root);
        let loc = ArticleLocator::parse("1111.22222").unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let mut policy = AcquisitionPolicy::ensure_only();
        policy.allow_network = true;
        let err = acq.ensure_assets(&resolved, &policy).unwrap_err();
        assert!(matches!(err, AcquisitionError::Infrastructure(_)));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_non_arxiv_locator() {
        let acq = ArxivSourceAcquisition::new("/tmp");
        let loc = ArticleLocator::parse("doi:10.1/x").unwrap();
        assert!(!acq.can_resolve(&loc));
    }
}
