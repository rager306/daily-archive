//! Catalog HTML acquisition — offline ensure for textbook/HTML units (ADR-056 §2.11).
//!
//! Resolves `catalog:<source_code>/html/<unit_key>` and bare unit keys under a
//! configured source_code (e.g. gnn_textbook). Primary is HTML; no graph edges.

use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Offline catalog HTML provider (gnn_textbook, stanford HTML, …).
#[derive(Debug, Clone)]
pub struct CatalogHtmlAcquisition {
    pub catalog_root: PathBuf,
    pub source_code: String,
}

impl CatalogHtmlAcquisition {
    pub fn new(catalog_root: impl Into<PathBuf>, source_code: impl Into<String>) -> Self {
        Self {
            catalog_root: catalog_root.into(),
            source_code: source_code.into(),
        }
    }

    fn unit_dir(&self, unit_key: &str) -> PathBuf {
        // Preferred layout: article_catalog/<source>/html/<unit_key>/
        self.catalog_root
            .join("article_catalog")
            .join(&self.source_code)
            .join("html")
            .join(unit_key)
    }

    fn find_html(&self, unit_key: &str) -> Option<PathBuf> {
        let dir = self.unit_dir(unit_key);
        if !dir.is_dir() {
            // Fallback: walk for chapter.html under source_code
            return find_unit_html(&self.catalog_root, &self.source_code, unit_key);
        }
        let candidates = [
            dir.join("source").join("chapter.html"),
            dir.join("source").join(format!("{unit_key}.html")),
            dir.join("article.html"),
            dir.join("source").join("index.html"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }
}

impl SourceAcquisitionPort for CatalogHtmlAcquisition {
    fn provider_code(&self) -> &str {
        &self.source_code
    }

    fn can_resolve(&self, locator: &ArticleLocator) -> bool {
        match locator {
            ArticleLocator::Catalog { source_code, .. } => source_code == &self.source_code,
            ArticleLocator::Raw { value } => {
                value.starts_with(&format!("{}:", self.source_code))
                    || value.contains(&self.source_code)
                    || value.starts_with("gnn-ch-")
            }
            ArticleLocator::Local { path } => path.ends_with(".html") || path.ends_with(".htm"),
            ArticleLocator::Arxiv { .. } => false,
        }
    }

    fn resolve(&self, locator: &ArticleLocator) -> Result<ResolvedArticleRef, AcquisitionError> {
        if !self.can_resolve(locator) {
            return Err(AcquisitionError::CannotResolve {
                provider: self.source_code.clone(),
                detail: format!("locator {}", locator.kind_name()),
            });
        }
        let (unit_key, catalog_path) = match locator {
            ArticleLocator::Catalog {
                catalog_path,
                source_code,
            } => {
                if source_code != &self.source_code {
                    return Err(AcquisitionError::CannotResolve {
                        provider: self.source_code.clone(),
                        detail: format!("source_code={source_code}"),
                    });
                }
                let unit = catalog_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(catalog_path)
                    .to_string();
                (unit, Some(catalog_path.clone()))
            }
            ArticleLocator::Raw { value } => {
                // Accept bare unit keys like gnn-ch-about
                let unit = value.rsplit('/').next().unwrap_or(value).trim().to_string();
                (unit, None)
            }
            ArticleLocator::Local { path } => {
                let unit = Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("local-html")
                    .to_string();
                (unit, Some(path.clone()))
            }
            ArticleLocator::Arxiv { .. } => {
                return Err(AcquisitionError::CannotResolve {
                    provider: self.source_code.clone(),
                    detail: "arxiv not supported by html provider".into(),
                });
            }
        };

        Ok(ResolvedArticleRef {
            source_code: self.source_code.clone(),
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
        // Local path pin
        if let ArticleLocator::Local { path } = &resolved.locator {
            let p = PathBuf::from(path);
            if p.is_file() {
                return ensured_html(&self.catalog_root, &p, AssetDisposition::CatalogHit, true);
            }
            return Err(AcquisitionError::MissingAsset(path.clone()));
        }

        if let Some(html) = self.find_html(&resolved.unit_key) {
            return ensured_html(
                &self.catalog_root,
                &html,
                AssetDisposition::CatalogHit,
                true,
            );
        }

        if !policy.allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "HTML for {} / {} not under catalog; network textbook fetch not in this wave",
                self.source_code, resolved.unit_key
            )));
        }
        Err(AcquisitionError::Infrastructure(format!(
            "network fetch for {} HTML units not implemented (ensure offline catalog first)",
            self.source_code
        )))
    }
}

fn ensured_html(
    catalog_root: &Path,
    html: &Path,
    disposition: AssetDisposition,
    catalog_hit: bool,
) -> Result<EnsuredAssets, AcquisitionError> {
    let meta = fs::metadata(html)
        .map_err(|e| AcquisitionError::Infrastructure(format!("stat {}: {e}", html.display())))?;
    let sha = sha256_file(html)
        .map_err(|e| AcquisitionError::Infrastructure(format!("hash {}: {e}", html.display())))?;
    let path_str = html
        .strip_prefix(catalog_root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| html.to_string_lossy().into_owned());
    Ok(EnsuredAssets {
        primary: EnsuredPrimary {
            kind: PrimaryContentKind::Html,
            path: path_str.clone(),
            sha256: Some(sha.clone()),
            bytes: Some(meta.len()),
        },
        assets: vec![EnsuredAsset {
            kind: "html".into(),
            path: path_str,
            sha256: Some(sha),
            bytes: Some(meta.len()),
            disposition,
        }],
        catalog_hit,
    })
}

fn find_unit_html(root: &Path, source_code: &str, unit_key: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 50_000 {
            break;
        }
        let rd = fs::read_dir(&dir).ok()?;
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some(unit_key) {
                    let chapter = path.join("source").join("chapter.html");
                    if chapter.is_file() {
                        return Some(chapter);
                    }
                }
                // prune unrelated source trees lightly
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "arxiv" && source_code != "arxiv" {
                    continue;
                }
                stack.push(path);
            }
        }
    }
    None
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;
    use std::io::Write;

    #[test]
    fn catalog_pin_html_hit() {
        let root = std::env::temp_dir().join(format!("da-html-acq-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let unit = "gnn-ch-chapters__01-intro-to-graphs";
        let dir = root
            .join("article_catalog")
            .join("gnn_textbook")
            .join("html")
            .join(unit)
            .join("source");
        fs::create_dir_all(&dir).unwrap();
        let html = dir.join("chapter.html");
        let mut f = fs::File::create(&html).unwrap();
        f.write_all(b"<html><h1>Intro</h1></html>").unwrap();

        let acq = CatalogHtmlAcquisition::new(&root, "gnn_textbook");
        let loc = ArticleLocator::parse(&format!("catalog:gnn_textbook/html/{unit}")).unwrap();
        assert!(acq.can_resolve(&loc));
        let resolved = acq.resolve(&loc).unwrap();
        let assets = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap();
        assert!(assets.catalog_hit);
        assert_eq!(assets.primary.kind, PrimaryContentKind::Html);
        assert_eq!(assets.assets[0].disposition, AssetDisposition::CatalogHit);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bare_unit_key_resolve() {
        let acq = CatalogHtmlAcquisition::new("/tmp", "gnn_textbook");
        let loc = ArticleLocator::parse("gnn-ch-about").unwrap();
        // Raw locator
        assert!(matches!(loc, ArticleLocator::Raw { .. }));
        assert!(acq.can_resolve(&loc));
    }

    #[test]
    fn miss_without_network() {
        let root = std::env::temp_dir().join(format!("da-html-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let acq = CatalogHtmlAcquisition::new(&root, "gnn_textbook");
        let loc = ArticleLocator::parse("catalog:gnn_textbook/html/missing-chapter").unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap_err();
        assert!(matches!(err, AcquisitionError::NetworkDenied(_)));
        let _ = fs::remove_dir_all(&root);
    }
}
