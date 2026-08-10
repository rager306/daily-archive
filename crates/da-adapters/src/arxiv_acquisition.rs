//! ArXiv source acquisition adapter (ADR-056).
//!
//! Offline: catalog_hit when `{id}.pdf` exists under catalog_root.
//! Network: temporary process-boundary to `scripts/download_arxiv_to_catalog.py`
//! (never called from da-application). After success, re-walk catalog for PDF.

use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PROVIDER: &str = "arxiv";
const DOWNLOAD_SCRIPT: &str = "scripts/download_arxiv_to_catalog.py";

/// Filesystem-backed arXiv acquisition.
#[derive(Debug, Clone)]
pub struct ArxivSourceAcquisition {
    /// Root containing `article_catalog/...` (typically `data/article_catalog`).
    pub catalog_root: PathBuf,
    /// Repo root for spawning the recovery script (default: cwd).
    pub repo_root: PathBuf,
    /// Override download script path (tests).
    pub download_script: PathBuf,
}

impl ArxivSourceAcquisition {
    pub fn new(catalog_root: impl Into<PathBuf>) -> Self {
        let catalog_root = catalog_root.into();
        Self {
            catalog_root,
            repo_root: PathBuf::from("."),
            download_script: PathBuf::from(DOWNLOAD_SCRIPT),
        }
    }

    pub fn with_repo_root(mut self, repo_root: impl Into<PathBuf>) -> Self {
        self.repo_root = repo_root.into();
        self
    }

    fn find_pdf(&self, arxiv_id: &str) -> Option<PathBuf> {
        let name = format!("{arxiv_id}.pdf");
        find_named_file(&self.catalog_root, &name)
    }

    fn ensured_from_pdf(
        &self,
        pdf: &Path,
        disposition: AssetDisposition,
        catalog_hit: bool,
    ) -> Result<EnsuredAssets, AcquisitionError> {
        let meta = fs::metadata(pdf).map_err(|e| {
            AcquisitionError::Infrastructure(format!("stat {}: {e}", pdf.display()))
        })?;
        let bytes = meta.len();
        let sha = sha256_file(pdf).map_err(|e| {
            AcquisitionError::Infrastructure(format!("hash {}: {e}", pdf.display()))
        })?;
        let path_str = path_for_report(&self.catalog_root, pdf);
        let mut assets = vec![EnsuredAsset {
            kind: "pdf".into(),
            path: path_str.clone(),
            sha256: Some(sha.clone()),
            bytes: Some(bytes),
            disposition,
        }];

        // Optional HTML sibling (article.html next to unit root).
        if let Some(html) = sibling_html(pdf)
            && let Ok(hmeta) = fs::metadata(&html)
        {
            let hsha = sha256_file(&html).ok();
            assets.push(EnsuredAsset {
                kind: "html".into(),
                path: path_for_report(&self.catalog_root, &html),
                sha256: hsha,
                bytes: Some(hmeta.len()),
                disposition: if catalog_hit {
                    AssetDisposition::CatalogHit
                } else {
                    AssetDisposition::Fetched
                },
            });
        }

        Ok(EnsuredAssets {
            primary: EnsuredPrimary {
                kind: PrimaryContentKind::Pdf,
                path: path_str,
                sha256: Some(sha),
                bytes: Some(bytes),
            },
            assets,
            catalog_hit,
        })
    }

    fn fetch_via_script(&self, arxiv_id: &str, include_tex: bool) -> Result<(), AcquisitionError> {
        let script = if self.download_script.is_absolute() {
            self.download_script.clone()
        } else {
            self.repo_root.join(&self.download_script)
        };
        if !script.is_file() {
            return Err(AcquisitionError::Infrastructure(format!(
                "download script missing: {}",
                script.display()
            )));
        }

        let report_dir = self.repo_root.join("artifacts/source_recovery");
        let _ = fs::create_dir_all(&report_dir);
        let report_path = report_dir.join(format!("{arxiv_id}.json"));

        let mut cmd = Command::new("python3");
        cmd.current_dir(&self.repo_root)
            .arg(&script)
            .arg("--allow-network")
            .arg("--report")
            .arg(&report_path)
            .arg(arxiv_id);
        if include_tex {
            cmd.arg("--include-tex-source");
        }

        let output = cmd.output().map_err(|e| {
            AcquisitionError::Infrastructure(format!("spawn {}: {e}", script.display()))
        })?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AcquisitionError::Infrastructure(format!(
                "download failed for {arxiv_id} (exit={:?}): stdout={stdout} stderr={stderr}",
                output.status.code()
            )));
        }
        Ok(())
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
            return self.ensured_from_pdf(&pdf, AssetDisposition::CatalogHit, true);
        }

        if !policy.allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "PDF for {} not under {}; pass --allow-network to fetch",
                resolved.unit_key,
                self.catalog_root.display()
            )));
        }

        if policy.dry_run {
            return Err(AcquisitionError::MissingAsset(format!(
                "dry_run: would fetch {} via network",
                resolved.unit_key
            )));
        }

        self.fetch_via_script(&resolved.unit_key, policy.include_tex_source)?;

        match self.find_pdf(&resolved.unit_key) {
            Some(pdf) => self.ensured_from_pdf(&pdf, AssetDisposition::Fetched, false),
            None => Err(AcquisitionError::MissingAsset(format!(
                "download finished but PDF still missing for {}",
                resolved.unit_key
            ))),
        }
    }
}

fn sibling_html(pdf: &Path) -> Option<PathBuf> {
    // .../source/{id}.pdf → .../article.html
    let unit_root = pdf.parent()?.parent()?;
    let html = unit_root.join("article.html");
    if html.is_file() { Some(html) } else { None }
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
    fn network_missing_script_is_infrastructure_error() {
        let root = std::env::temp_dir().join(format!("da-arxiv-acq-net-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let mut acq = ArxivSourceAcquisition::new(&root);
        acq.download_script = root.join("no-such-script.py");
        acq.repo_root = root.clone();
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
