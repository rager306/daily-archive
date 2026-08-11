//! ArXiv source acquisition adapter (ADR-056).
//!
//! Offline: catalog_hit when `{id}.pdf` exists under catalog_root.
//! Network: native reqwest fetch (PDF + HTML + Atom meta) into canonical catalog
//! layout — **no Python process boundary**.

use crate::http_fetch::{http_get_bytes, http_get_first_valid};
use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER: &str = "arxiv";
const UA: &str = "daily-archive/arxiv-acquisition (research bot)";

/// Filesystem-backed arXiv acquisition.
#[derive(Debug, Clone)]
pub struct ArxivSourceAcquisition {
    /// Root containing `article_catalog/...` (typically `data/article_catalog`).
    pub catalog_root: PathBuf,
    pub user_agent: String,
}

impl ArxivSourceAcquisition {
    pub fn new(catalog_root: impl Into<PathBuf>) -> Self {
        Self {
            catalog_root: catalog_root.into(),
            user_agent: UA.into(),
        }
    }

    /// Kept for CLI composition compatibility (repo root no longer required).
    pub fn with_repo_root(self, _repo_root: impl Into<PathBuf>) -> Self {
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

    /// Native fetch: Atom meta → category dir → PDF (+ optional HTML).
    fn fetch_native(
        &self,
        arxiv_id: &str,
        include_html: bool,
    ) -> Result<PathBuf, AcquisitionError> {
        let meta = fetch_atom_meta(arxiv_id, &self.user_agent);
        let category = meta.category;
        let title = meta.title;

        let unit_root = self
            .catalog_root
            .join("article_catalog")
            .join("arxiv")
            .join(&category)
            .join(arxiv_id);
        let source_dir = unit_root.join("source");
        fs::create_dir_all(&source_dir).map_err(|e| {
            AcquisitionError::Infrastructure(format!("mkdir {}: {e}", source_dir.display()))
        })?;

        let pdf_path = source_dir.join(format!("{arxiv_id}.pdf"));
        if !pdf_path.is_file() {
            let urls = [
                format!("https://arxiv.org/pdf/{arxiv_id}.pdf"),
                format!("https://arxiv.org/pdf/{arxiv_id}"),
                format!("https://export.arxiv.org/pdf/{arxiv_id}"),
            ];
            let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();
            let (src_url, bytes) =
                http_get_first_valid(&url_refs, Some(&self.user_agent), validate_pdf)?;
            atomic_write(&pdf_path, &bytes)?;
            let _ = src_url;
        } else {
            // Validate existing
            let existing = fs::read(&pdf_path).map_err(|e| {
                AcquisitionError::Infrastructure(format!("read {}: {e}", pdf_path.display()))
            })?;
            validate_pdf(&existing).map_err(|e| {
                AcquisitionError::Infrastructure(format!("existing PDF invalid: {e}"))
            })?;
        }

        let html_path = unit_root.join("article.html");
        if include_html && !html_path.is_file() {
            let html_urls = [
                format!("https://arxiv.org/html/{arxiv_id}"),
                format!("https://ar5iv.labs.arxiv.org/html/{arxiv_id}"),
            ];
            let hrefs: Vec<&str> = html_urls.iter().map(String::as_str).collect();
            if let Ok((_url, bytes)) =
                http_get_first_valid(&hrefs, Some(&self.user_agent), validate_html)
            {
                let _ = atomic_write(&html_path, &bytes);
            }
            // HTML is optional sidecar — ignore fetch failure.
        }

        let pdf_bytes = fs::read(&pdf_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("read {}: {e}", pdf_path.display()))
        })?;
        let sha = sha256_bytes(&pdf_bytes);
        write_article_json(
            &unit_root,
            arxiv_id,
            &category,
            &title,
            &sha,
            pdf_bytes.len() as u64,
        )?;

        Ok(pdf_path)
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

        if policy.include_tex_source {
            // TeX optional later; PDF/HTML primary path is product-critical.
            // Do not fail ensure solely because TeX is requested.
        }

        let pdf = self.fetch_native(&resolved.unit_key, policy.include_html)?;
        self.ensured_from_pdf(&pdf, AssetDisposition::Fetched, false)
    }
}

// ─── Meta / validation / IO ─────────────────────────────────────────────────

struct AtomMeta {
    category: String,
    title: String,
}

fn fetch_atom_meta(arxiv_id: &str, ua: &str) -> AtomMeta {
    let url = format!("https://export.arxiv.org/api/query?id_list={arxiv_id}");
    match http_get_bytes(&url, Some(ua)) {
        Ok(bytes) => parse_atom_meta(&String::from_utf8_lossy(&bytes)),
        Err(_) => AtomMeta {
            category: "unknown".into(),
            title: String::new(),
        },
    }
}

fn parse_atom_meta(xml: &str) -> AtomMeta {
    // Lightweight parse — no full XML crate dependency required for one field.
    let title = extract_tag(xml, "title")
        .map(|t| collapse_ws(&t))
        .filter(|t| {
            !t.eq_ignore_ascii_case("ArXiv Query: Search Authors results for ...")
                && t != "http://arxiv.org/api/s"
        })
        .unwrap_or_default();
    // Prefer arxiv:primary_category term="cs.AI"
    let category = extract_attr(xml, "primary_category", "term")
        .or_else(|| extract_attr(xml, "category", "term"))
        .map(|t| normalize_category(&t))
        .unwrap_or_else(|| "unknown".into());
    // First <title> in feed is often the query title — prefer entry title.
    let entry_title = xml
        .split("<entry")
        .nth(1)
        .and_then(|entry| extract_tag(entry, "title"))
        .map(|t| collapse_ws(&t))
        .unwrap_or(title);
    AtomMeta {
        category,
        title: entry_title,
    }
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after = &xml[start..];
    let gt = after.find('>')?;
    let content_start = gt + 1;
    let end = after[content_start..].find(&close)?;
    Some(after[content_start..content_start + end].to_string())
}

fn extract_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let needle = format!("<{tag}");
    let start = xml.find(&needle)?;
    let after = &xml[start..];
    let end = after.find('>')?;
    let tag_body = &after[..end];
    let attr_pat = format!("{attr}=\"");
    let a = tag_body.find(&attr_pat)?;
    let rest = &tag_body[a + attr_pat.len()..];
    let close = rest.find('"')?;
    Some(rest[..close].to_string())
}

fn normalize_category(value: &str) -> String {
    let n = value.trim().to_ascii_lowercase().replace('.', "-");
    if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !n.is_empty() {
        n
    } else {
        "unknown".into()
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_pdf(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 1000 {
        return Err(format!("too small ({} bytes)", bytes.len()));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err("missing %PDF- magic".into());
    }
    Ok(())
}

fn validate_html(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 200 {
        return Err(format!("too small ({} bytes)", bytes.len()));
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(400)]).to_ascii_lowercase();
    if !head.contains("<html") && !head.contains("<!doctype") {
        return Err("not HTML".into());
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AcquisitionError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AcquisitionError::Infrastructure(format!("mkdir {}: {e}", parent.display()))
        })?;
    }
    let tmp = path.with_extension("partial");
    fs::write(&tmp, bytes)
        .map_err(|e| AcquisitionError::Infrastructure(format!("write {}: {e}", tmp.display())))?;
    fs::rename(&tmp, path)
        .map_err(|e| AcquisitionError::Infrastructure(format!("rename {}: {e}", path.display())))?;
    Ok(())
}

fn write_article_json(
    unit_root: &Path,
    arxiv_id: &str,
    category: &str,
    title: &str,
    pdf_sha: &str,
    pdf_bytes: u64,
) -> Result<(), AcquisitionError> {
    let catalog_path = format!("arxiv/{category}/{arxiv_id}");
    let title = if title.is_empty() {
        arxiv_id.to_string()
    } else {
        title.to_string()
    };
    let article = serde_json::json!({
        "schema_version": "article.v00.01",
        "article_key": arxiv_id,
        "catalog_path": catalog_path,
        "source_code": "arxiv",
        "source_type": "preprint",
        "publisher": "arXiv",
        "coarse_topic_code": category,
        "identity": {
            "title": title,
            "authors": [],
            "canonical_url": format!("https://arxiv.org/abs/{arxiv_id}"),
            "citation_key": arxiv_id.replace('.', "_")
        },
        "domain_profile": "scientific_paper",
        "source_strategy": {
            "primary_source_variant_id": format!("{arxiv_id}:source:pdf"),
            "preferred_content_order": ["pdf", "html"],
            "pdf_policy": "primary",
            "parser_readiness": "grobid_pdf",
            "graph_readiness": "not_claimed"
        },
        "source_variants": [{
            "variant_id": format!("{arxiv_id}:source:pdf"),
            "source_role": "primary_pdf",
            "source_format": "pdf",
            "is_primary": true,
            "path": format!("source/{arxiv_id}.pdf"),
            "sha256": pdf_sha,
            "byte_size": pdf_bytes,
            "capture_status": "captured_local",
            "network_fetch_attempted": true
        }],
        "safety_flags": {
            "trusted_kg_import_allowed": false,
            "production_ladybugdb_write_allowed": false
        }
    });
    let path = unit_root.join("article.json");
    let bytes = serde_json::to_vec_pretty(&article)
        .map_err(|e| AcquisitionError::Infrastructure(format!("serialize article.json: {e}")))?;
    atomic_write(&path, &bytes)
}

fn sibling_html(pdf: &Path) -> Option<PathBuf> {
    let unit_root = pdf.parent()?.parent()?;
    let html = unit_root.join("article.html");
    if html.is_file() { Some(html) } else { None }
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
    fn rejects_non_arxiv_locator() {
        let acq = ArxivSourceAcquisition::new("/tmp");
        let loc = ArticleLocator::parse("doi:10.1/x").unwrap();
        assert!(!acq.can_resolve(&loc));
    }

    #[test]
    fn parse_atom_meta_category() {
        let xml = r#"<?xml version="1.0"?>
        <feed>
          <title>ArXiv Query</title>
          <entry>
            <title>DocMemo: Example Paper</title>
            <arxiv:primary_category term="cs.AI" xmlns:arxiv="http://arxiv.org/schemas/atom"/>
            <category term="cs.AI"/>
          </entry>
        </feed>"#;
        let m = parse_atom_meta(xml);
        assert_eq!(m.category, "cs-ai");
        assert!(m.title.contains("DocMemo"));
    }

    #[test]
    fn validate_pdf_rejects_html() {
        assert!(validate_pdf(b"<html>not a pdf</html>").is_err());
        let mut good = b"%PDF-1.4 ".to_vec();
        good.extend(std::iter::repeat_n(b'x', 1200));
        assert!(validate_pdf(&good).is_ok());
    }

    #[test]
    fn normalize_category_dot_to_dash() {
        assert_eq!(normalize_category("cs.CL"), "cs-cl");
        assert_eq!(normalize_category("bad/cat"), "unknown");
    }
}
