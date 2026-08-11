//! Catalog HTML acquisition — textbook/HTML units (ADR-056 §2.11).
//!
//! Offline: catalog_hit when `chapter.html` exists.
//! Network: fetch from DATA TOC (`source_catalogs/*_toc.yaml`) when
//! `allow_network` — writes atomic catalog unit (article.json + source/chapter.html).
//! No graph edges. Application never calls HTTP.

use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Offline + optional network catalog HTML provider.
#[derive(Debug, Clone)]
pub struct CatalogHtmlAcquisition {
    pub catalog_root: PathBuf,
    pub source_code: String,
    /// Optional TOC path (e.g. data/source_catalogs/gnn_textbook_toc.yaml).
    pub toc_path: Option<PathBuf>,
    pub repo_root: PathBuf,
    pub user_agent: String,
}

impl CatalogHtmlAcquisition {
    pub fn new(catalog_root: impl Into<PathBuf>, source_code: impl Into<String>) -> Self {
        let source_code = source_code.into();
        let default_toc = PathBuf::from(format!("data/source_catalogs/{source_code}_toc.yaml"));
        Self {
            catalog_root: catalog_root.into(),
            source_code,
            toc_path: if default_toc.is_file() {
                Some(default_toc)
            } else {
                None
            },
            repo_root: PathBuf::from("."),
            user_agent: "daily-archive/load-plane".into(),
        }
    }

    pub fn with_toc_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.toc_path = Some(path.into());
        self
    }

    pub fn with_repo_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.repo_root = root.into();
        self
    }

    fn unit_dir(&self, unit_key: &str) -> PathBuf {
        self.catalog_root
            .join("article_catalog")
            .join(&self.source_code)
            .join("html")
            .join(unit_key)
    }

    fn find_html(&self, unit_key: &str) -> Option<PathBuf> {
        let dir = self.unit_dir(unit_key);
        if !dir.is_dir() {
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

    fn lookup_toc_unit(&self, unit_key: &str) -> Result<Option<TocUnit>, AcquisitionError> {
        let Some(toc_path) = &self.toc_path else {
            return Ok(None);
        };
        let path = if toc_path.is_absolute() {
            toc_path.clone()
        } else {
            self.repo_root.join(toc_path)
        };
        if !path.is_file() {
            return Ok(None);
        }
        let toc = load_toc(&path)?;
        if toc.source_code != self.source_code {
            return Err(AcquisitionError::Infrastructure(format!(
                "TOC source_code={} != provider {}",
                toc.source_code, self.source_code
            )));
        }
        Ok(toc.units.into_iter().find(|u| u.unit_key == unit_key))
    }

    fn fetch_and_materialize(
        &self,
        unit: &TocUnit,
        toc: &SourceCatalogToc,
    ) -> Result<PathBuf, AcquisitionError> {
        let url = format!(
            "{}{}",
            toc.base_url.trim_end_matches('/'),
            if unit.url_path.starts_with('/') {
                unit.url_path.clone()
            } else {
                format!("/{}", unit.url_path)
            }
        );
        let bytes = http_get_bytes(&url, &self.user_agent)?;
        if bytes.len() < 64 {
            return Err(AcquisitionError::Infrastructure(format!(
                "fetched HTML too small for {} ({} bytes)",
                unit.unit_key,
                bytes.len()
            )));
        }
        // Reject obvious non-HTML
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).to_ascii_lowercase();
        if !head.contains("<html") && !head.contains("<!doctype") {
            return Err(AcquisitionError::Infrastructure(format!(
                "response for {} does not look like HTML",
                unit.unit_key
            )));
        }

        let unit_dir = self.unit_dir(&unit.unit_key);
        let source_dir = unit_dir.join("source");
        fs::create_dir_all(&source_dir).map_err(|e| {
            AcquisitionError::Infrastructure(format!("mkdir {}: {e}", source_dir.display()))
        })?;

        let html_path = source_dir.join("chapter.html");
        let tmp = source_dir.join("chapter.html.partial");
        fs::write(&tmp, &bytes).map_err(|e| {
            AcquisitionError::Infrastructure(format!("write {}: {e}", tmp.display()))
        })?;
        fs::rename(&tmp, &html_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("rename {}: {e}", html_path.display()))
        })?;

        let sha = sha256_bytes(&bytes);
        let article = build_article_json(toc, unit, &url, &sha, bytes.len() as u64);
        let article_path = unit_dir.join("article.json");
        let article_tmp = unit_dir.join("article.json.partial");
        let article_bytes = serde_json::to_vec_pretty(&article).map_err(|e| {
            AcquisitionError::Infrastructure(format!("serialize article.json: {e}"))
        })?;
        fs::write(&article_tmp, article_bytes).map_err(|e| {
            AcquisitionError::Infrastructure(format!("write {}: {e}", article_tmp.display()))
        })?;
        fs::rename(&article_tmp, &article_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("rename {}: {e}", article_path.display()))
        })?;

        Ok(html_path)
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
                "HTML for {} / {} not under catalog; pass --allow-network to fetch from TOC",
                self.source_code, resolved.unit_key
            )));
        }
        if policy.dry_run {
            return Err(AcquisitionError::MissingAsset(format!(
                "dry_run: would fetch HTML for {}",
                resolved.unit_key
            )));
        }

        let unit = self.lookup_toc_unit(&resolved.unit_key)?.ok_or_else(|| {
            AcquisitionError::MissingAsset(format!(
                "unit {} not in TOC {:?} and not on disk",
                resolved.unit_key, self.toc_path
            ))
        })?;
        let toc_path = self.toc_path.as_ref().ok_or_else(|| {
            AcquisitionError::Infrastructure("TOC path required for network fetch".into())
        })?;
        let toc_full = if toc_path.is_absolute() {
            toc_path.clone()
        } else {
            self.repo_root.join(toc_path)
        };
        let toc = load_toc(&toc_full)?;
        let html = self.fetch_and_materialize(&unit, &toc)?;
        ensured_html(&self.catalog_root, &html, AssetDisposition::Fetched, false)
    }
}

// ─── TOC DATA ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SourceCatalogToc {
    pub schema_version: String,
    pub source_code: String,
    pub container: String,
    pub publisher: String,
    pub base_url: String,
    pub units: Vec<TocUnit>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TocUnit {
    pub slug: String,
    pub unit_key: String,
    pub title: String,
    pub url_path: String,
}

pub fn load_toc(path: &Path) -> Result<SourceCatalogToc, AcquisitionError> {
    let text = fs::read_to_string(path).map_err(|e| {
        AcquisitionError::Infrastructure(format!("read TOC {}: {e}", path.display()))
    })?;
    serde_yaml::from_str(&text)
        .map_err(|e| AcquisitionError::Infrastructure(format!("parse TOC {}: {e}", path.display())))
}

/// List unit keys from TOC file (CLI batch helper).
pub fn toc_unit_keys(path: &Path) -> Result<Vec<String>, AcquisitionError> {
    Ok(load_toc(path)?
        .units
        .into_iter()
        .map(|u| u.unit_key)
        .collect())
}

fn build_article_json(
    toc: &SourceCatalogToc,
    unit: &TocUnit,
    url: &str,
    sha: &str,
    byte_size: u64,
) -> serde_json::Value {
    let catalog_path = format!("{}/html/{}", toc.source_code, unit.unit_key);
    let citation_key = unit.unit_key.replace('-', "_");
    serde_json::json!({
        "schema_version": "article.v00.01",
        "article_key": unit.unit_key,
        "catalog_path": catalog_path,
        "source_code": toc.source_code,
        "source_type": "textbook",
        "publisher": toc.publisher,
        "coarse_topic_code": "html",
        "topic_tags": ["GNN", "textbook", "graph-neural-networks"],
        "identity": {
            "title": unit.title,
            "authors": [toc.publisher],
            "container": toc.container,
            "canonical_url": url,
            "citation_key": citation_key
        },
        "domain_profile": "textbook",
        "source_strategy": {
            "primary_source_variant_id": format!("{}:source:html-chapter", unit.unit_key),
            "preferred_content_order": ["textbook_html_chapter"],
            "metadata_order": ["html_meta", "manual"],
            "pdf_policy": "no_pdf_expected_for_html_chapter",
            "fallback_policy": "manual_review_when_html_missing_or_low_quality",
            "parser_readiness": "local_html_via_universal_source",
            "chunk_readiness": "structure_loaded_source",
            "graph_readiness": "not_claimed"
        },
        "source_variants": [{
            "variant_id": format!("{}:source:html-chapter", unit.unit_key),
            "source_role": "textbook_html_chapter",
            "source_format": "html",
            "source_origin": format!("{}_site", toc.source_code),
            "is_primary": true,
            "is_content_bearing": true,
            "is_metadata_only": false,
            "path": "source/chapter.html",
            "url": url,
            "media_type": "text/html",
            "sha256": sha,
            "byte_size": byte_size,
            "capture_status": "captured_local",
            "capture_policy": "bounded_fetch_into_catalog",
            "loader_outcome": "not_loaded",
            "requires_conversion": false,
            "conversion_hint": null,
            "raw_text_embedded": false,
            "raw_binary_embedded": false,
            "network_fetch_attempted": true,
            "local_path": "source/chapter.html",
            "parser_readiness_claimed": false
        }],
        "expected_profile": {
            "should_load": true,
            "should_parse_text": true,
            "should_chunk": true,
            "should_have_arxiv_metadata": false,
            "domain_profile": "textbook",
            "known_risks": [
                "html_nav_boilerplate_may_need_cleanup",
                "not_graph_import_eligible"
            ]
        },
        "safety_flags": {
            "trusted_kg_import_allowed": false,
            "production_ladybugdb_write_allowed": false,
            "raw_text_embedded_in_metadata": false
        }
    })
}

fn http_get_bytes(url: &str, user_agent: &str) -> Result<Vec<u8>, AcquisitionError> {
    // Process-boundary HTTP (adapters only). curl is available in ops env;
    // avoids requiring reqwest blocking feature.
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "60", "-A", user_agent, url])
        .output()
        .map_err(|e| AcquisitionError::Infrastructure(format!("spawn curl: {e}")))?;
    if !output.status.success() {
        return Err(AcquisitionError::Infrastructure(format!(
            "curl failed for {url} (exit={:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
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
    Ok(sha256_bytes(&data))
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
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

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bare_unit_key_resolve() {
        let acq = CatalogHtmlAcquisition::new("/tmp", "gnn_textbook");
        let loc = ArticleLocator::parse("gnn-ch-about").unwrap();
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

    #[test]
    fn load_repo_toc_if_present() {
        let path = Path::new("data/source_catalogs/gnn_textbook_toc.yaml");
        if !path.is_file() {
            return;
        }
        let toc = load_toc(path).unwrap();
        assert_eq!(toc.source_code, "gnn_textbook");
        assert!(toc.units.len() >= 27);
        assert!(toc.units.iter().any(|u| u.unit_key == "gnn-ch-about"));
        assert!(
            toc.units
                .iter()
                .any(|u| u.unit_key == "gnn-ch-chapters__06-gnn-foundations")
        );
    }
}
