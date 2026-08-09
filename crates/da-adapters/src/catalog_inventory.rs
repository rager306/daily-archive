//! Filesystem adapter for deterministic, read-only catalog reconciliation.

use da_domain::catalog::{CatalogFinding, CatalogFindingKind};
use da_ports::{CatalogInventoryError, CatalogInventoryPort, CatalogInventorySnapshot};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct FilesystemCatalogInventory {
    root: PathBuf,
}

impl FilesystemCatalogInventory {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl CatalogInventoryPort for FilesystemCatalogInventory {
    fn inventory(&self) -> Result<CatalogInventorySnapshot, CatalogInventoryError> {
        let mut files = Vec::new();
        collect_files(&self.root, &mut files).map_err(err)?;
        files.sort();
        let metadata: Vec<_> = files
            .iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "article.json"))
            .cloned()
            .collect();
        let pdfs: Vec<_> = files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
            .cloned()
            .collect();
        let htmls: Vec<_> = files
            .iter()
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n == "article.html" || n == "chapter.html")
            })
            .cloned()
            .collect();
        let mut findings = Vec::new();
        let mut identities: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for path in &metadata {
            match serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).map_err(err)?) {
                Ok(value) => {
                    let raw = value
                        .get("article_key")
                        .and_then(|v| v.as_str())
                        .or_else(|| value.pointer("/identity/arxiv_id").and_then(|v| v.as_str()));
                    let Some(raw) = raw else {
                        findings.push(finding(
                            CatalogFindingKind::InvalidMetadata,
                            "unknown",
                            path,
                            &self.root,
                            "missing article_key and identity.arxiv_id",
                        ));
                        continue;
                    };
                    let id = canonical_id(raw);
                    identities.entry(id.clone()).or_default().push(path.clone());
                    if let Some(variants) = value.get("source_variants").and_then(|v| v.as_array())
                    {
                        for variant in variants {
                            if let Some(raw_path) = variant.get("path").and_then(|v| v.as_str())
                                && resolve_variant_path(&self.root, path, raw_path).is_none()
                            {
                                findings.push(finding(
                                    CatalogFindingKind::MissingVariantAsset,
                                    &id,
                                    path,
                                    &self.root,
                                    raw_path,
                                ));
                            }
                        }
                    }
                }
                Err(error) => findings.push(finding(
                    CatalogFindingKind::InvalidMetadata,
                    "unknown",
                    path,
                    &self.root,
                    &error.to_string(),
                )),
            }
        }
        for (id, paths) in &identities {
            if paths.len() > 1 {
                findings.push(finding(
                    CatalogFindingKind::DuplicateIdentity,
                    id,
                    &paths[0],
                    &self.root,
                    &format!("{} metadata records", paths.len()),
                ));
            }
        }
        let mut asset_roots = BTreeSet::new();
        for asset in pdfs.iter().chain(htmls.iter()) {
            if let Some(root) = article_root(asset) {
                asset_roots.insert(root);
            }
        }
        for root in asset_roots {
            if !root.join("article.json").exists() {
                let id = root
                    .file_name()
                    .and_then(|v| v.to_str())
                    .map(canonical_id)
                    .unwrap_or_else(|| "unknown".into());
                findings.push(finding(
                    CatalogFindingKind::MissingArticleMetadata,
                    &id,
                    &root,
                    &self.root,
                    "source asset exists without article.json",
                ));
            }
        }
        for (id, paths) in pdfs_by_identity(&pdfs) {
            let hashes: BTreeSet<_> = paths
                .iter()
                .map(|p| sha256(p))
                .collect::<Result<_, _>>()
                .map_err(err)?;
            if hashes.len() > 1 {
                findings.push(finding(
                    CatalogFindingKind::ConflictingIdentityAssets,
                    &id,
                    &paths[0],
                    &self.root,
                    &format!("{} distinct PDF checksums", hashes.len()),
                ));
            }
        }
        findings.sort_by(|a, b| {
            (format!("{:?}", a.kind), &a.canonical_id, &a.path).cmp(&(
                format!("{:?}", b.kind),
                &b.canonical_id,
                &b.path,
            ))
        });
        Ok(CatalogInventorySnapshot {
            article_metadata_count: metadata.len(),
            pdf_count: pdfs.len(),
            html_count: htmls.len(),
            canonical_identity_count: identities.len(),
            findings,
        })
    }
}

fn resolve_variant_path(catalog_root: &Path, metadata_path: &Path, value: &str) -> Option<PathBuf> {
    let raw = Path::new(value);
    let mut candidates = vec![metadata_path.parent()?.join(raw), catalog_root.join(raw)];
    if let Ok(relative) = raw.strip_prefix("data/article_catalog") {
        candidates.push(catalog_root.join(relative));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn canonical_id(value: &str) -> String {
    let value = value.trim().trim_start_matches("arXiv:");
    if let Some((base, suffix)) = value.rsplit_once('v')
        && !base.is_empty()
        && !suffix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_digit())
    {
        return base.to_string();
    }
    value.to_string()
}
fn article_root(path: &Path) -> Option<PathBuf> {
    let mut p = path.parent()?;
    if p.file_name().is_some_and(|v| v == "source") {
        p = p.parent()?;
    }
    Some(p.to_path_buf())
}
fn pdfs_by_identity(paths: &[PathBuf]) -> BTreeMap<String, Vec<PathBuf>> {
    let mut out = BTreeMap::new();
    for path in paths {
        let identity = article_root(path)
            .and_then(|root| root.file_name().map(|value| value.to_owned()))
            .and_then(|value| value.to_str().map(canonical_id))
            .or_else(|| {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .map(canonical_id)
            });
        if let Some(identity) = identity {
            out.entry(identity)
                .or_insert_with(Vec::new)
                .push(path.clone());
        }
    }
    out
}
fn sha256(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
fn collect_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(root)? {
        let p = e?.path();
        if p.is_dir() {
            collect_files(&p, out)?
        } else {
            out.push(p)
        }
    }
    Ok(())
}
fn err(error: impl std::fmt::Display) -> CatalogInventoryError {
    CatalogInventoryError::Infrastructure(error.to_string())
}
fn finding(
    kind: CatalogFindingKind,
    id: &str,
    path: &Path,
    root: &Path,
    evidence: &str,
) -> CatalogFinding {
    CatalogFinding {
        kind,
        canonical_id: id.into(),
        path: path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string(),
        evidence: evidence.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("da-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn reports_missing_metadata_stale_variant_and_duplicate_identity() {
        let r = root("drift");
        let a = r.join("article_catalog/arxiv/cs-ai/1234.5678v1");
        std::fs::create_dir_all(a.join("source")).unwrap();
        std::fs::write(a.join("source/1234.5678v1.pdf"), b"one").unwrap();
        std::fs::write(
            a.join("article.json"),
            r#"{"article_key":"1234.5678v1","source_variants":[{"path":"missing.pdf"}]}"#,
        )
        .unwrap();
        let b = r.join("article_catalog/arxiv/cs-ai/1234.5678v2");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("article.json"), r#"{"article_key":"1234.5678v2"}"#).unwrap();
        let c = r.join("article_catalog/arxiv/cs-ai/9999.0001/source");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("9999.0001.pdf"), b"pdf").unwrap();
        let s = FilesystemCatalogInventory::new(&r).inventory().unwrap();
        assert!(
            s.findings
                .iter()
                .any(|f| f.kind == CatalogFindingKind::MissingVariantAsset)
        );
        assert!(
            s.findings
                .iter()
                .any(|f| f.kind == CatalogFindingKind::DuplicateIdentity)
        );
        assert!(
            s.findings
                .iter()
                .any(|f| f.kind == CatalogFindingKind::MissingArticleMetadata)
        );
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn original_pdf_names_are_scoped_by_article_directory() {
        let r = root("original-scoped");
        for (id, bytes) in [("1234.5678v1", b"one" as &[u8]), ("9999.0001v1", b"two")] {
            let p = r.join(format!("arxiv/cs-ai/{id}/source"));
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("original.pdf"), bytes).unwrap();
        }
        let s = FilesystemCatalogInventory::new(&r).inventory().unwrap();
        assert!(
            !s.findings
                .iter()
                .any(|f| f.kind == CatalogFindingKind::ConflictingIdentityAssets)
        );
        std::fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn conflicting_versions_fail_visible() {
        let r = root("conflict");
        for (v, bytes) in [("v1", b"one" as &[u8]), ("v2", b"two")] {
            let p = r.join(format!("arxiv/cs-ai/1234.5678{v}/source"));
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join(format!("1234.5678{v}.pdf")), bytes).unwrap();
        }
        let s = FilesystemCatalogInventory::new(&r).inventory().unwrap();
        assert!(
            s.findings
                .iter()
                .any(|f| f.kind == CatalogFindingKind::ConflictingIdentityAssets)
        );
        std::fs::remove_dir_all(r).unwrap();
    }
}
