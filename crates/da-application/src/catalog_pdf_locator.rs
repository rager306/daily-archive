//! Filesystem `CatalogPdfLocator` — catalog truth only (M411 S01).
//!
//! Walks `DA_CATALOG_ROOT` (injected path; default `data/sources/arxiv`)
//! looking for `source/<id>.pdf` or a versioned sibling `source/<id>vN.pdf`.
//! Never invents a path: a miss is `None`.

use std::fs;
use std::path::{Path, PathBuf};

use da_ports::CatalogPdfLocator;

/// Locate a catalogued PDF under a catalog root.
pub struct FilesystemCatalogPdfLocator {
    catalog_root: PathBuf,
}

impl FilesystemCatalogPdfLocator {
    pub fn new(catalog_root: impl Into<PathBuf>) -> Self {
        Self {
            catalog_root: catalog_root.into(),
        }
    }

    /// Composition-root helper: `DA_CATALOG_ROOT` or `data/sources/arxiv`.
    pub fn from_env() -> Self {
        let root =
            std::env::var("DA_CATALOG_ROOT").unwrap_or_else(|_| "data/sources/arxiv".to_string());
        Self::new(root)
    }
}

impl CatalogPdfLocator for FilesystemCatalogPdfLocator {
    fn locate(&self, arxiv_id: &str) -> Option<PathBuf> {
        let id = arxiv_id.trim();
        if id.is_empty() || id.contains(['/', '\\', '\0']) {
            return None;
        }
        let exact_name = format!("{id}.pdf");
        let versioned_prefix = format!("{id}v");
        let mut exact: Option<PathBuf> = None;
        let mut versioned: Vec<(u32, PathBuf)> = Vec::new();
        walk_pdfs(&self.catalog_root, &mut |path| {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                return;
            };
            let Some(parent) = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            else {
                return;
            };
            if parent != "source" {
                return;
            }
            if name.eq_ignore_ascii_case(&exact_name) {
                exact = Some(path.to_path_buf());
                return;
            }
            if let Some(n) = name
                .strip_prefix(&versioned_prefix)
                .and_then(|s| s.strip_suffix(".pdf"))
                .and_then(|rest| rest.parse::<u32>().ok())
            {
                versioned.push((n, path.to_path_buf()));
            }
        });
        if let Some(path) = exact {
            return Some(path);
        }
        versioned.sort_by_key(|(n, _)| *n);
        versioned.pop().map(|(_, path)| path)
    }
}

fn walk_pdfs(root: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_pdfs(&path, visit);
        } else if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            visit(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pdf(root: &Path, rel: &str) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"%PDF-1.4 test").unwrap();
        path
    }

    #[test]
    fn locate_prefers_exact_source_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let exact = write_pdf(
            dir.path(),
            "article_catalog/arxiv/cs-ai/2506.07853/source/2506.07853.pdf",
        );
        write_pdf(
            dir.path(),
            "article_catalog/arxiv/cs-ai/2506.07853/source/2506.07853v1.pdf",
        );
        let locator = FilesystemCatalogPdfLocator::new(dir.path());
        assert_eq!(
            locator.locate("2506.07853").as_deref(),
            Some(exact.as_path())
        );
    }

    #[test]
    fn locate_falls_back_to_highest_versioned_pdf() {
        let dir = tempfile::tempdir().unwrap();
        write_pdf(
            dir.path(),
            "article_catalog/arxiv/cs-ai/2506.07853/source/2506.07853v1.pdf",
        );
        let v2 = write_pdf(
            dir.path(),
            "article_catalog/arxiv/cs-ai/2506.07853/source/2506.07853v2.pdf",
        );
        let locator = FilesystemCatalogPdfLocator::new(dir.path());
        assert_eq!(locator.locate("2506.07853").as_deref(), Some(v2.as_path()));
    }

    #[test]
    fn locate_returns_none_when_catalog_has_no_pdf() {
        let dir = tempfile::tempdir().unwrap();
        write_pdf(
            dir.path(),
            "article_catalog/arxiv/cs-ai/1111.11111/source/1111.11111.pdf",
        );
        let locator = FilesystemCatalogPdfLocator::new(dir.path());
        assert!(locator.locate("2506.07853").is_none());
        assert!(locator.locate("").is_none());
        assert!(locator.locate("../escape").is_none());
    }
}
