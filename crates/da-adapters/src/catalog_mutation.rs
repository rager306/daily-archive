//! Filesystem implementation of the catalog mutation port.

use da_domain::catalog::{CatalogAssetDescriptor, CatalogAssetKind};
use da_ports::{CatalogInventoryError, CatalogMutationPort, CatalogWriteDisposition};
use sha2::{Digest, Sha256};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct FilesystemCatalogMutation {
    root: PathBuf,
}
impl FilesystemCatalogMutation {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl CatalogMutationPort for FilesystemCatalogMutation {
    fn read_metadata(&self, relative_path: &str) -> Result<Vec<u8>, CatalogInventoryError> {
        let path = self.safe_existing(relative_path)?;
        std::fs::read(path).map_err(err)
    }
    fn describe_assets(
        &self,
        article_root: &str,
    ) -> Result<Vec<CatalogAssetDescriptor>, CatalogInventoryError> {
        let root = self.safe_existing(article_root)?;
        if !root.is_dir() {
            return Err(err("article root is not a directory"));
        }
        let catalog_root = self.root.canonicalize().map_err(err)?;
        let mut files = Vec::new();
        collect_files(&root, &mut files).map_err(err)?;
        files.sort();
        let mut out = Vec::new();
        for path in files {
            let kind = if path
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
            {
                Some(CatalogAssetKind::Pdf)
            } else if path
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v == "article.html" || v == "chapter.html")
            {
                Some(CatalogAssetKind::Html)
            } else {
                None
            };
            if let Some(kind) = kind {
                let bytes = std::fs::read(&path).map_err(err)?;
                out.push(CatalogAssetDescriptor {
                    kind,
                    relative_path: path
                        .strip_prefix(&catalog_root)
                        .map_err(err)?
                        .display()
                        .to_string(),
                    sha256: hex::encode(Sha256::digest(&bytes)),
                    byte_size: bytes.len() as u64,
                });
            }
        }
        Ok(out)
    }
    fn write_metadata_atomic_if_absent(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<CatalogWriteDisposition, CatalogInventoryError> {
        let target = self.safe_target(relative_path)?;
        if target.exists() {
            let current = std::fs::read(&target).map_err(err)?;
            return if current == bytes {
                Ok(CatalogWriteDisposition::Unchanged)
            } else {
                Err(err(format!(
                    "target exists with different content: {}",
                    target.display()
                )))
            };
        }
        let parent = target
            .parent()
            .ok_or_else(|| err("target parent missing"))?;
        create_dir_all(parent).map_err(err)?;
        let canonical_parent = parent.canonicalize().map_err(err)?;
        let canonical_root = self.root.canonicalize().map_err(err)?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(err("target parent escapes catalog root"));
        }
        let seq = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = target
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| err("target filename invalid"))?;
        let tmp = parent.join(format!(".{name}.tmp-{}-{seq}", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(err)?;
            file.write_all(bytes).map_err(err)?;
            file.sync_all().map_err(err)?;
            std::fs::rename(&tmp, &target).map_err(err)?;
            Ok(CatalogWriteDisposition::Applied)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
    fn write_file_atomic(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<CatalogWriteDisposition, CatalogInventoryError> {
        let target = self.safe_target(relative_path)?;
        if target.exists() && std::fs::read(&target).map_err(err)? == bytes {
            return Ok(CatalogWriteDisposition::Unchanged);
        }
        let parent = target
            .parent()
            .ok_or_else(|| err("target parent missing"))?;
        create_dir_all(parent).map_err(err)?;
        let root = self.root.canonicalize().map_err(err)?;
        if !parent.canonicalize().map_err(err)?.starts_with(root) {
            return Err(err("target parent escapes catalog root"));
        }
        let seq = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = target
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| err("target filename invalid"))?;
        let tmp = parent.join(format!(".{name}.tmp-{}-{seq}", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(err)?;
            file.write_all(bytes).map_err(err)?;
            file.sync_all().map_err(err)?;
            std::fs::rename(&tmp, &target).map_err(err)?;
            Ok(CatalogWriteDisposition::Applied)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}
impl FilesystemCatalogMutation {
    fn safe_target(&self, value: &str) -> Result<PathBuf, CatalogInventoryError> {
        let p = Path::new(value);
        if p.is_absolute()
            || p.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(err("catalog path traversal rejected"));
        }
        Ok(self.root.join(p))
    }
    fn safe_existing(&self, value: &str) -> Result<PathBuf, CatalogInventoryError> {
        let p = self.safe_target(value)?;
        let canonical = p.canonicalize().map_err(err)?;
        let root = self.root.canonicalize().map_err(err)?;
        if !canonical.starts_with(root) {
            return Err(err("catalog symlink escape rejected"));
        }
        Ok(canonical)
    }
}
fn collect_files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let p = entry?.path();
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

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "da-catalog-mutation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn writes_atomically_and_replays_unchanged() {
        let r = root();
        let m = FilesystemCatalogMutation::new(&r);
        let path = "arxiv/x/article.json";
        assert_eq!(
            m.write_metadata_atomic_if_absent(path, b"{}\n").unwrap(),
            CatalogWriteDisposition::Applied
        );
        assert_eq!(
            m.write_metadata_atomic_if_absent(path, b"{}\n").unwrap(),
            CatalogWriteDisposition::Unchanged
        );
        assert!(
            m.write_metadata_atomic_if_absent(path, b"different")
                .is_err()
        );
        assert!(
            std::fs::read_dir(r.join("arxiv/x")).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp-"))
        );
        std::fs::remove_dir_all(r).unwrap()
    }
    #[test]
    fn atomic_file_update_replaces_then_replays_unchanged() {
        let r = root();
        let m = FilesystemCatalogMutation::new(&r);
        let path = "index.json";
        assert_eq!(
            m.write_file_atomic(path, b"one").unwrap(),
            CatalogWriteDisposition::Applied
        );
        assert_eq!(
            m.write_file_atomic(path, b"two").unwrap(),
            CatalogWriteDisposition::Applied
        );
        assert_eq!(
            m.write_file_atomic(path, b"two").unwrap(),
            CatalogWriteDisposition::Unchanged
        );
        assert_eq!(std::fs::read(r.join(path)).unwrap(), b"two");
        std::fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn rejects_traversal() {
        let r = root();
        let m = FilesystemCatalogMutation::new(&r);
        assert!(
            m.write_metadata_atomic_if_absent("../escape.json", b"x")
                .is_err()
        );
        assert!(!r.parent().unwrap().join("escape.json").exists());
        std::fs::remove_dir_all(r).unwrap()
    }
    #[test]
    fn describes_live_pdf_and_html() {
        let r = root();
        let a = r.join("arxiv/x");
        std::fs::create_dir_all(a.join("source")).unwrap();
        std::fs::write(a.join("source/x.pdf"), b"pdf").unwrap();
        std::fs::write(a.join("article.html"), b"html").unwrap();
        let d = FilesystemCatalogMutation::new(&r)
            .describe_assets("arxiv/x")
            .unwrap();
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|x| x.sha256.len() == 64));
        std::fs::remove_dir_all(r).unwrap()
    }
}
