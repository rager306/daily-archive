//! ArXiv source acquisition adapter (ADR-056).
//!
//! Offline: catalog_hit when `{id}.pdf` exists under catalog_root.
//! Network: native reqwest fetch (PDF + HTML + Atom meta) into canonical catalog
//! layout — **no Python process boundary**.

use crate::http_fetch::{http_get_bytes, http_get_first_valid};
use da_domain::{
    AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind,
    try_parse_arxiv_locator,
};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER: &str = "arxiv";
const UA: &str = "daily-archive/arxiv-acquisition (research bot)";

/// Provider endpoint overrides (D239: provider roles in env, not code).
/// Defaults match production arXiv; tests override via env + wiremock.
fn export_base() -> String {
    std::env::var("DA_ARXIV_EXPORT_BASE_URL").unwrap_or_else(|_| "https://export.arxiv.org".into())
}
fn abs_base() -> String {
    std::env::var("DA_ARXIV_ABS_BASE_URL").unwrap_or_else(|_| "https://arxiv.org".into())
}
fn ar5iv_base() -> String {
    std::env::var("DA_AR5IV_BASE_URL").unwrap_or_else(|_| "https://ar5iv.labs.arxiv.org".into())
}

/// Filesystem-backed arXiv acquisition.
#[derive(Debug, Clone)]
pub struct ArxivSourceAcquisition {
    /// Root containing `article_catalog/...` (typically `data/sources/arxiv`).
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

    fn find_pdf(&self, source_key: &str) -> Option<PathBuf> {
        let name = format!("{source_key}.pdf");
        find_named_file(&self.catalog_root, &name)
    }

    fn ensured_from_pdf(
        &self,
        pdf: &Path,
        disposition: AssetDisposition,
        catalog_hit: bool,
        extra_assets: Vec<EnsuredAsset>,
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

        assets.extend(extra_assets);

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

    /// Native fetch: Atom meta → category dir → PDF (+ optional HTML/TeX).
    fn fetch_native(
        &self,
        arxiv_id: &str,
        source_key: &str,
        revision: Option<u32>,
        include_html: bool,
        include_tex: bool,
    ) -> Result<(PathBuf, Vec<EnsuredAsset>), AcquisitionError> {
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

        let pdf_path = source_dir.join(format!("{source_key}.pdf"));
        let abs_base = abs_base();
        let export_base = export_base();
        let requested_pdf_url = format!("{abs_base}/pdf/{source_key}");
        let pdf_resolved_url = if !pdf_path.is_file() {
            let urls = [
                format!("{abs_base}/pdf/{source_key}"),
                format!("{abs_base}/pdf/{source_key}.pdf"),
                format!("{export_base}/pdf/{source_key}"),
            ];
            let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();
            let (src_url, bytes) =
                http_get_first_valid(&url_refs, Some(&self.user_agent), validate_pdf)?;
            atomic_write(&pdf_path, &bytes)?;
            src_url
        } else {
            // Validate existing
            let existing = fs::read(&pdf_path).map_err(|e| {
                AcquisitionError::Infrastructure(format!("read {}: {e}", pdf_path.display()))
            })?;
            validate_pdf(&existing).map_err(|e| {
                AcquisitionError::Infrastructure(format!("existing PDF invalid: {e}"))
            })?;
            requested_pdf_url.clone()
        };

        let html_name = revision
            .map(|value| format!("article.v{value}.html"))
            .unwrap_or_else(|| "article.html".into());
        let html_path = unit_root.join(html_name);
        let ar5iv_base = ar5iv_base();
        let requested_html_url = format!("{abs_base}/html/{source_key}");
        let mut html_resolved_url = None;
        if include_html && !html_path.is_file() {
            let html_urls = [
                requested_html_url.clone(),
                format!("{ar5iv_base}/html/{source_key}"),
            ];
            let hrefs: Vec<&str> = html_urls.iter().map(String::as_str).collect();
            match http_get_first_valid(&hrefs, Some(&self.user_agent), validate_html) {
                Ok((url, bytes)) => {
                    atomic_write(&html_path, &bytes)?;
                    html_resolved_url = Some(url);
                }
                Err(error) => {
                    tracing::warn!(%arxiv_id, %source_key, %error, "optional versioned HTML sidecar unavailable");
                }
            }
        } else if html_path.is_file() {
            html_resolved_url = Some(requested_html_url.clone());
        }

        let mut extra = Vec::new();
        if include_tex {
            match ensure_tex_source(
                &unit_root,
                source_key,
                &self.catalog_root,
                &self.user_agent,
                true,
            ) {
                Ok(assets) => extra.extend(assets),
                Err(e) => {
                    // TeX is optional sidecar: PDF ensure still succeeds.
                    tracing::warn!(%arxiv_id, error = %e, "tex source ensure skipped");
                }
            }
        }

        let pdf_bytes = fs::read(&pdf_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("read {}: {e}", pdf_path.display()))
        })?;
        let sha = sha256_bytes(&pdf_bytes);
        let html_capture = if html_path.is_file() {
            let bytes = fs::read(&html_path).map_err(|e| {
                AcquisitionError::Infrastructure(format!("read {}: {e}", html_path.display()))
            })?;
            Some(CapturedHtml {
                path: html_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("article.html")
                    .to_string(),
                sha256: sha256_bytes(&bytes),
                bytes: bytes.len() as u64,
                requested_url: requested_html_url,
                fetched_from_url: html_resolved_url,
            })
        } else {
            None
        };
        write_article_json(
            &unit_root,
            arxiv_id,
            source_key,
            revision,
            &category,
            &title,
            &sha,
            pdf_bytes.len() as u64,
            &requested_pdf_url,
            &pdf_resolved_url,
            html_capture.as_ref(),
        )?;

        Ok((pdf_path, extra))
    }

    fn ensure_tex_sidecars(
        &self,
        pdf: &Path,
        arxiv_id: &str,
        allow_network: bool,
    ) -> Vec<EnsuredAsset> {
        let Some(unit_root) = pdf.parent().and_then(|p| p.parent()) else {
            return Vec::new();
        };
        match ensure_tex_source(
            unit_root,
            arxiv_id,
            &self.catalog_root,
            &self.user_agent,
            allow_network,
        ) {
            Ok(assets) => assets,
            Err(e) => {
                tracing::warn!(%arxiv_id, error = %e, "tex source ensure skipped");
                Vec::new()
            }
        }
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
            ArticleLocator::Arxiv { id, .. } => (id.clone(), None),
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
            source_profile: Some("scientific_paper".into()),
            container_ref: None,
        })
    }

    fn ensure_assets(
        &self,
        resolved: &ResolvedArticleRef,
        policy: &AcquisitionPolicy,
    ) -> Result<EnsuredAssets, AcquisitionError> {
        let source_key = resolved.locator.source_key();
        let revision = resolved.locator.arxiv_revision();
        if let Some(pdf) = self.find_pdf(&source_key) {
            let extra = if policy.include_tex_source && !policy.dry_run {
                self.ensure_tex_sidecars(&pdf, &source_key, policy.allow_network)
            } else {
                Vec::new()
            };
            return self.ensured_from_pdf(&pdf, AssetDisposition::CatalogHit, true, extra);
        }

        if !policy.allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "PDF for exact source {} not under {}; pass --allow-network to fetch",
                source_key,
                self.catalog_root.display()
            )));
        }

        if policy.dry_run {
            return Err(AcquisitionError::MissingAsset(format!(
                "dry_run: would fetch exact source {} via network",
                source_key
            )));
        }

        let (pdf, extra) = self.fetch_native(
            &resolved.unit_key,
            &source_key,
            revision,
            policy.include_html,
            policy.include_tex_source,
        )?;
        self.ensured_from_pdf(&pdf, AssetDisposition::Fetched, false, extra)
    }
}

// ─── TeX source sidecar (optional; never blocks PDF primary) ─────────────────

// Caps mirrored from the archived Python downloader (archive/python-era-cold-s04/).
const MAX_TEX_ARCHIVE_BYTES: u64 = 100 << 20;
const MAX_TEX_MEMBERS: usize = 1000;
/// Per-member and expanded-total caps enforced when full member streams are hashed
/// (FilesystemTexArchive path). Ensure-time listing uses archive size + member count.
#[allow(dead_code)]
const MAX_TEX_MEMBER_BYTES: u64 = 20 << 20;
#[allow(dead_code)]
const MAX_TEX_TOTAL_BYTES: u64 = 100 << 20;
const TEX_INVENTORY_SCHEMA: &str = "arxiv-tex-source-inventory.v1";

fn ensure_tex_source(
    unit_root: &Path,
    arxiv_id: &str,
    catalog_root: &Path,
    user_agent: &str,
    allow_network: bool,
) -> Result<Vec<EnsuredAsset>, AcquisitionError> {
    let source_dir = unit_root.join("source");
    fs::create_dir_all(&source_dir).map_err(|e| {
        AcquisitionError::Infrastructure(format!("mkdir {}: {e}", source_dir.display()))
    })?;
    let tar_path = source_dir.join(format!("{arxiv_id}.tar.gz"));
    let inventory_path = source_dir.join(format!("{arxiv_id}.tex-inventory.json"));

    let mut disposition = AssetDisposition::CatalogHit;
    if !tar_path.is_file() {
        if !allow_network {
            return Err(AcquisitionError::NetworkDenied(format!(
                "TeX archive for {arxiv_id} missing; pass --allow-network"
            )));
        }
        let abs_base = abs_base();
        let url = format!("{abs_base}/src/{arxiv_id}");
        let bytes = http_get_bytes(&url, Some(user_agent))?;
        if bytes.len() as u64 > MAX_TEX_ARCHIVE_BYTES || bytes.is_empty() {
            return Err(AcquisitionError::Infrastructure(format!(
                "TeX archive size invalid for {arxiv_id} ({} bytes)",
                bytes.len()
            )));
        }
        // Stage then inspect before publish.
        let tmp = tar_path.with_extension("tar.gz.partial");
        fs::write(&tmp, &bytes).map_err(|e| {
            AcquisitionError::Infrastructure(format!("write {}: {e}", tmp.display()))
        })?;
        inspect_tex_archive(&tmp).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            AcquisitionError::Infrastructure(format!("unsafe TeX archive for {arxiv_id}: {e}"))
        })?;
        fs::rename(&tmp, &tar_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("rename {}: {e}", tar_path.display()))
        })?;
        disposition = AssetDisposition::Fetched;
    } else {
        inspect_tex_archive(&tar_path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("existing TeX archive unsafe: {e}"))
        })?;
    }

    let inv = build_tex_inventory(arxiv_id, &tar_path, catalog_root)?;
    let inv_bytes = serde_json::to_vec_pretty(&inv)
        .map_err(|e| AcquisitionError::Infrastructure(format!("serialize tex inventory: {e}")))?;
    atomic_write(&inventory_path, &inv_bytes)?;

    let tar_meta = fs::metadata(&tar_path).map_err(|e| {
        AcquisitionError::Infrastructure(format!("stat {}: {e}", tar_path.display()))
    })?;
    let tar_sha = sha256_file(&tar_path).map_err(|e| {
        AcquisitionError::Infrastructure(format!("hash {}: {e}", tar_path.display()))
    })?;
    let inv_sha = sha256_file(&inventory_path).map_err(|e| {
        AcquisitionError::Infrastructure(format!("hash {}: {e}", inventory_path.display()))
    })?;

    Ok(vec![
        EnsuredAsset {
            kind: "tex_tar".into(),
            path: path_for_report(catalog_root, &tar_path),
            sha256: Some(tar_sha),
            bytes: Some(tar_meta.len()),
            disposition,
        },
        EnsuredAsset {
            kind: "tex_inventory".into(),
            path: path_for_report(catalog_root, &inventory_path),
            sha256: Some(inv_sha),
            bytes: Some(inv_bytes.len() as u64),
            disposition,
        },
    ])
}

/// Safe inventory via `tar -tzvf` listing + size caps (no extract to disk).
fn inspect_tex_archive(path: &Path) -> Result<(), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() == 0 || meta.len() > MAX_TEX_ARCHIVE_BYTES {
        return Err(format!("invalid size {}", meta.len()));
    }
    let output = std::process::Command::new("tar")
        .args(["-tzf"])
        .arg(path)
        .output()
        .map_err(|e| format!("spawn tar: {e}"))?;
    if !output.status.success() {
        // try uncompressed / auto
        let output2 = std::process::Command::new("tar")
            .args(["-tf"])
            .arg(path)
            .output()
            .map_err(|e| format!("spawn tar -tf: {e}"))?;
        if !output2.status.success() {
            return Err(format!(
                "tar list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return validate_tar_names(&String::from_utf8_lossy(&output2.stdout));
    }
    validate_tar_names(&String::from_utf8_lossy(&output.stdout))
}

fn validate_tar_names(listing: &str) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut count = 0usize;
    for line in listing.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        count += 1;
        if count > MAX_TEX_MEMBERS {
            return Err("member cap exceeded".into());
        }
        if name.starts_with('/')
            || name.starts_with('\\')
            || name.contains("..")
            || name.contains('\\')
            || name.contains('\0')
            || name.starts_with('-')
        {
            return Err(format!("unsafe member name: {name:?}"));
        }
        if !seen.insert(name.to_string()) {
            return Err(format!("duplicate member: {name:?}"));
        }
    }
    if count == 0 {
        return Err("empty archive listing".into());
    }
    Ok(())
}

fn build_tex_inventory(
    arxiv_id: &str,
    tar_path: &Path,
    catalog_root: &Path,
) -> Result<serde_json::Value, AcquisitionError> {
    let archive_bytes = fs::metadata(tar_path)
        .map_err(|e| AcquisitionError::Infrastructure(e.to_string()))?
        .len();
    let archive_sha =
        sha256_file(tar_path).map_err(|e| AcquisitionError::Infrastructure(e.to_string()))?;
    let listing = {
        let out = std::process::Command::new("tar")
            .args(["-tzf"])
            .arg(tar_path)
            .output();
        match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            _ => {
                let o = std::process::Command::new("tar")
                    .args(["-tf"])
                    .arg(tar_path)
                    .output()
                    .map_err(|e| AcquisitionError::Infrastructure(e.to_string()))?;
                if !o.status.success() {
                    return Err(AcquisitionError::Infrastructure(
                        "tar listing failed for inventory".into(),
                    ));
                }
                String::from_utf8_lossy(&o.stdout).into_owned()
            }
        }
    };
    validate_tar_names(&listing).map_err(AcquisitionError::Infrastructure)?;
    let mut files = Vec::new();
    let mut directory_count = 0u64;
    let mut tex_count = 0u64;
    let mut bib_count = 0u64;
    for name in listing.lines().map(str::trim).filter(|n| !n.is_empty()) {
        if name.ends_with('/') {
            directory_count += 1;
            continue;
        }
        let suffix = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| format!(".{}", s.to_ascii_lowercase()))
            .unwrap_or_default();
        if suffix == ".tex" {
            tex_count += 1;
        }
        if suffix == ".bib" {
            bib_count += 1;
        }
        // Member digests require stream extract; inventory lists paths only for ensure.
        // Full member hashes remain available via FilesystemTexArchive after ensure.
        files.push(serde_json::json!({
            "path": name,
            "byte_size": null,
            "sha256": null,
            "suffix": suffix,
        }));
    }
    let archive_path = path_for_report(catalog_root, tar_path);
    Ok(serde_json::json!({
        "schema_version": TEX_INVENTORY_SCHEMA,
        "article_id": arxiv_id,
        "import_eligible": false,
        "archive_path": archive_path,
        "sha256": archive_sha,
        "byte_size": archive_bytes,
        "member_count": files.len() as u64 + directory_count,
        "file_count": files.len(),
        "directory_count": directory_count,
        "tex_file_count": tex_count,
        "bib_file_count": bib_count,
        "expanded_bytes": null,
        "files": files,
        "note": "member digests deferred; archive-level sha256 is authoritative for ensure"
    }))
}

// ─── Meta / validation / IO ─────────────────────────────────────────────────

struct AtomMeta {
    category: String,
    title: String,
}

fn fetch_atom_meta(arxiv_id: &str, ua: &str) -> AtomMeta {
    let export_base = export_base();
    let url = format!("{export_base}/api/query?id_list={arxiv_id}");
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

struct CapturedHtml {
    path: String,
    sha256: String,
    bytes: u64,
    requested_url: String,
    fetched_from_url: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn write_article_json(
    unit_root: &Path,
    arxiv_id: &str,
    source_key: &str,
    revision: Option<u32>,
    category: &str,
    title: &str,
    pdf_sha: &str,
    pdf_bytes: u64,
    requested_pdf_url: &str,
    resolved_pdf_url: &str,
    html: Option<&CapturedHtml>,
) -> Result<(), AcquisitionError> {
    let catalog_path = format!("arxiv/{category}/{arxiv_id}");
    let title = if title.is_empty() {
        arxiv_id.to_string()
    } else {
        title.to_string()
    };
    let variant_suffix = revision
        .map(|value| format!("v{value}"))
        .unwrap_or_else(|| "latest".into());
    let pdf_variant_id = format!("{arxiv_id}:source:{variant_suffix}:pdf");
    let mut source_variants = vec![serde_json::json!({
        "variant_id": pdf_variant_id,
        "source_role": "primary_pdf",
        "source_format": "pdf",
        "is_primary": true,
        "path": format!("source/{source_key}.pdf"),
        "sha256": pdf_sha,
        "byte_size": pdf_bytes,
        "capture_status": "captured_local",
        "network_fetch_attempted": true,
        "requested_url": requested_pdf_url,
        "fetched_from_url": resolved_pdf_url,
        "requested_revision": revision
    })];
    if let Some(html) = html {
        source_variants.push(serde_json::json!({
            "variant_id": format!("{arxiv_id}:source:{variant_suffix}:html"),
            "source_role": "alternate_html",
            "source_format": "html",
            "is_primary": false,
            "path": html.path,
            "sha256": html.sha256,
            "byte_size": html.bytes,
            "capture_status": "captured_local",
            "network_fetch_attempted": true,
            "requested_url": html.requested_url,
            "fetched_from_url": html.fetched_from_url,
            "requested_revision": revision
        }));
    }
    let article = serde_json::json!({
        "schema_version": "article.v00.02",
        "article_key": arxiv_id,
        "catalog_path": catalog_path,
        "source_code": "arxiv",
        "source_type": "preprint",
        "publisher": "arXiv",
        "coarse_topic_code": category,
        "topic_tags": [category, "catalog-acquisition"],
        "identity": {
            "title": title,
            "authors": [],
            "canonical_url": format!("https://arxiv.org/abs/{arxiv_id}"),
            "citation_key": arxiv_id.replace('.', "_")
        },
        "requested_source": {
            "source_key": source_key,
            "revision": revision,
            "versioned": revision.is_some()
        },
        "domain_profile": "scientific_paper",
        "source_strategy": {
            "primary_source_variant_id": pdf_variant_id,
            "preferred_content_order": ["pdf", "html"],
            "pdf_policy": "primary",
            "parser_readiness": "grobid_pdf",
            "graph_readiness": "not_claimed"
        },
        "source_variants": source_variants,
        "safety_flags": {
            "graph_import_allowed": false,
            "trusted_kg_import_allowed": false,
            "production_import_attempted": false,
            "production_ladybugdb_write_allowed": false,
            "raw_binary_embedded_in_metadata": false,
            "raw_text_embedded_in_metadata": false
        },
        "safety_defaults": {
            "external_network_authorized": false,
            "fact_promotion_authorized": false,
            "graph_writes_authorized": false,
            "llm_calls_authorized": false,
            "production_import_authorized": false
        },
        "acquisition": {
            "network_used": true,
            "import_eligible": false
        }
    });
    let metadata_name = revision
        .map(|value| format!("article.v{value}.json"))
        .unwrap_or_else(|| "article.json".into());
    let path = unit_root.join(&metadata_name);
    let bytes = serde_json::to_vec_pretty(&article)
        .map_err(|e| AcquisitionError::Infrastructure(format!("serialize {metadata_name}: {e}")))?;
    atomic_write(&path, &bytes)
}

fn sibling_html(pdf: &Path) -> Option<PathBuf> {
    let unit_root = pdf.parent()?.parent()?;
    let source_stem = pdf.file_stem()?.to_str()?;
    let revision = try_parse_arxiv_locator(source_stem)?.1;
    let name = revision
        .map(|value| format!("article.v{value}.html"))
        .unwrap_or_else(|| "article.html".into());
    let html = unit_root.join(name);
    html.is_file().then_some(html)
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
        write_source_fixture(root, arxiv_id, arxiv_id, pdf_bytes)
    }

    fn write_source_fixture(
        root: &Path,
        arxiv_id: &str,
        source_key: &str,
        pdf_bytes: &[u8],
    ) -> PathBuf {
        let dir = root
            .join("article_catalog")
            .join("arxiv")
            .join("cs-ai")
            .join(arxiv_id)
            .join("source");
        fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join(format!("{source_key}.pdf"));
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
        assert!(!assets.assets.iter().any(|a| a.kind == "tex_tar"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn versioned_request_never_reuses_bare_latest_pdf() {
        let root = std::env::temp_dir().join(format!(
            "da-arxiv-acq-version-isolation-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_fixture(&root, "2506.07853", b"%PDF-1.4 bare latest");

        let acq = ArxivSourceAcquisition::new(&root);
        let loc = ArticleLocator::parse("https://arxiv.org/html/2506.07853v1").unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        assert_eq!(resolved.unit_key, "2506.07853");
        assert_eq!(resolved.locator.source_key(), "2506.07853v1");
        let err = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap_err();
        assert!(matches!(err, AcquisitionError::NetworkDenied(_)));
        assert!(err.to_string().contains("2506.07853v1"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn versioned_catalog_hit_reports_versioned_pdf_and_html() {
        let root =
            std::env::temp_dir().join(format!("da-arxiv-acq-version-hit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let pdf = write_source_fixture(
            &root,
            "2506.07853",
            "2506.07853v1",
            b"%PDF-1.4 requested v1",
        );
        let html = pdf
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("article.v1.html");
        fs::write(&html, b"<!doctype html><title>v1</title>").unwrap();

        let acq = ArxivSourceAcquisition::new(&root);
        let loc = ArticleLocator::parse("2506.07853v1").unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let assets = acq
            .ensure_assets(&resolved, &AcquisitionPolicy::ensure_only())
            .unwrap();
        assert!(assets.catalog_hit);
        assert!(assets.primary.path.ends_with("source/2506.07853v1.pdf"));
        assert!(
            assets
                .assets
                .iter()
                .any(|asset| asset.path.ends_with("article.v1.html"))
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn versioned_metadata_is_separate_and_lists_captured_variants() {
        let root =
            std::env::temp_dir().join(format!("da-arxiv-acq-version-meta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("article.json"), b"{\"latest\":true}").unwrap();
        let html = CapturedHtml {
            path: "article.v1.html".into(),
            sha256: "html-sha".into(),
            bytes: 42,
            requested_url: "https://arxiv.org/html/2506.07853v1".into(),
            fetched_from_url: Some("https://arxiv.org/html/2506.07853v1".into()),
        };
        write_article_json(
            &root,
            "2506.07853",
            "2506.07853v1",
            Some(1),
            "cs-ai",
            "Example",
            "pdf-sha",
            100,
            "https://arxiv.org/pdf/2506.07853v1",
            "https://arxiv.org/pdf/2506.07853v1",
            Some(&html),
        )
        .unwrap();
        assert_eq!(
            fs::read(root.join("article.json")).unwrap(),
            b"{\"latest\":true}"
        );
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("article.v1.json")).unwrap()).unwrap();
        assert_eq!(value["article_key"], "2506.07853");
        assert_eq!(value["requested_source"]["revision"], 1);
        assert_eq!(value["requested_source"]["source_key"], "2506.07853v1");
        assert_eq!(value["source_variants"].as_array().unwrap().len(), 2);
        assert_eq!(value["source_variants"][0]["source_format"], "pdf");
        assert_eq!(value["source_variants"][1]["source_format"], "html");

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

    #[test]
    fn validate_tar_names_rejects_traversal() {
        assert!(validate_tar_names("../evil.tex\n").is_err());
        assert!(validate_tar_names("/abs/path.tex\n").is_err());
        assert!(validate_tar_names("ok/main.tex\nok/refs.bib\n").is_ok());
    }

    #[test]
    fn tex_inventory_schema_constant() {
        assert_eq!(TEX_INVENTORY_SCHEMA, "arxiv-tex-source-inventory.v1");
        assert_eq!(MAX_TEX_ARCHIVE_BYTES, 100 << 20);
        assert_eq!(MAX_TEX_MEMBERS, 1000);
        assert_eq!(MAX_TEX_MEMBER_BYTES, 20 << 20);
        assert_eq!(MAX_TEX_TOTAL_BYTES, 100 << 20);
    }
}
