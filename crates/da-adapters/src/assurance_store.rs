//! Filesystem ACAP report store with immutable, atomic per-article writes.

use da_domain::ArticleAssuranceReport;
use da_ports::{AssuranceReportStore, AssuranceStoreError};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct FilesystemAssuranceReportStore {
    root: PathBuf,
}

impl FilesystemAssuranceReportStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn report_path(&self, article_id: &str) -> Result<PathBuf, AssuranceStoreError> {
        if article_id.is_empty()
            || article_id == "."
            || article_id == ".."
            || article_id.contains(['/', '\\'])
            || article_id.chars().any(|character| character.is_control())
        {
            return Err(AssuranceStoreError::Storage(
                "unsafe article ID rejected".into(),
            ));
        }
        Ok(self.root.join(format!("{article_id}.json")))
    }

    fn read_existing(
        &self,
        path: &Path,
        expected_article_id: &str,
    ) -> Result<ArticleAssuranceReport, AssuranceStoreError> {
        let canonical_root = self.root.canonicalize().map_err(storage)?;
        let canonical_path = path.canonicalize().map_err(storage)?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(AssuranceStoreError::Storage(
                "assurance report symlink escape rejected".into(),
            ));
        }
        let stored: ArticleAssuranceReport =
            serde_json::from_slice(&std::fs::read(canonical_path).map_err(storage)?)
                .map_err(storage)?;
        let article_id = stored.article_id.clone();
        let normalized = stored.clone().normalize().map_err(|e| {
            use da_domain::AssuranceError;
            match e {
                AssuranceError::SchemaVersionDrift | AssuranceError::PolicyIdDrift => {
                    AssuranceStoreError::Integrity { article_id }
                }
                other => storage(other),
            }
        })?;
        if normalized.article_id != expected_article_id || normalized != stored {
            return Err(AssuranceStoreError::Integrity {
                article_id: normalized.article_id,
            });
        }
        Ok(normalized)
    }

    fn sync_parent(path: &Path) -> Result<(), AssuranceStoreError> {
        let parent = path
            .parent()
            .ok_or_else(|| AssuranceStoreError::Storage("report parent missing".into()))?;
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(storage)
    }
}

impl AssuranceReportStore for FilesystemAssuranceReportStore {
    fn load(
        &self,
        article_id: &str,
    ) -> Result<Option<ArticleAssuranceReport>, AssuranceStoreError> {
        let path = self.report_path(article_id)?;
        if path.exists() {
            self.read_existing(&path, article_id).map(Some)
        } else {
            Ok(None)
        }
    }

    fn store(&self, report: &ArticleAssuranceReport) -> Result<(), AssuranceStoreError> {
        let normalized = report.clone().normalize().map_err(storage)?;
        if normalized != *report {
            return Err(AssuranceStoreError::Integrity {
                article_id: report.article_id.clone(),
            });
        }
        let target = self.report_path(&report.article_id)?;
        let mut bytes = serde_json::to_vec_pretty(&normalized).map_err(storage)?;
        bytes.push(b'\n');
        create_dir_all(&self.root).map_err(storage)?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp = self.root.join(format!(
            ".{}.tmp-{}-{sequence}",
            report.article_id,
            std::process::id()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(storage)?;
            file.write_all(&bytes).map_err(storage)?;
            file.sync_all().map_err(storage)?;
            match std::fs::hard_link(&temp, &target) {
                Ok(()) => Self::sync_parent(&target),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing = self.read_existing(&target, &report.article_id)?;
                    if existing == normalized {
                        Ok(())
                    } else {
                        Err(AssuranceStoreError::Conflict {
                            article_id: report.article_id.clone(),
                        })
                    }
                }
                Err(error) => Err(storage(error)),
            }
        })();
        let _ = std::fs::remove_file(&temp);
        result
    }
}

fn storage(error: impl std::fmt::Display) -> AssuranceStoreError {
    AssuranceStoreError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{ArticleContextState, AssuranceVerdict, ContentDigest};
    use std::sync::{Arc, Barrier};

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "da-acap-store-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn report(policy: &str) -> ArticleAssuranceReport {
        ArticleAssuranceReport::new(
            "2507.19457",
            ContentDigest::of_text(policy),
            ArticleContextState::Materialized,
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn filesystem_store_is_atomic_idempotent_and_conflict_safe() {
        let root = temp_root();
        let store = FilesystemAssuranceReportStore::new(&root);
        let first = report("policy-v1");
        store.store(&first).unwrap();
        store.store(&first).unwrap();
        assert_eq!(store.load("2507.19457").unwrap(), Some(first));
        assert!(matches!(
            store.store(&report("policy-v2")),
            Err(AssuranceStoreError::Conflict { .. })
        ));
        assert!(!std::fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")
        }));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_key_must_match_embedded_article_identity() {
        let root = temp_root();
        let store = FilesystemAssuranceReportStore::new(&root);
        std::fs::write(
            root.join("other.json"),
            serde_json::to_vec_pretty(&report("policy-v1")).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            store.load("other"),
            Err(AssuranceStoreError::Integrity { .. })
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_store_rejects_tampered_derived_verdict() {
        let root = temp_root();
        let store = FilesystemAssuranceReportStore::new(&root);
        let expected = report("policy-v1");
        store.store(&expected).unwrap();
        let path = root.join("2507.19457.json");
        let mut payload: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        payload["verdict"] = serde_json::json!(AssuranceVerdict::Pass);
        payload["schema_version"] = serde_json::json!("forged");
        std::fs::write(path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
        assert!(matches!(
            store.load("2507.19457"),
            Err(AssuranceStoreError::Integrity { .. })
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_conflicting_first_write_never_overwrites() {
        let root = temp_root();
        let store = Arc::new(FilesystemAssuranceReportStore::new(&root));
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for policy in ["policy-v1", "policy-v2"] {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                store.store(&report(policy))
            }));
        }
        barrier.wait();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|result| matches!(result, Err(AssuranceStoreError::Conflict { .. })))
                .count(),
            1
        );
        let loaded = store.load("2507.19457").unwrap().unwrap();
        assert!(loaded == report("policy-v1") || loaded == report("policy-v2"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_store_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;
        let root = temp_root();
        let outside = root.with_extension("outside.json");
        std::fs::write(&outside, serde_json::to_vec(&report("policy-v1")).unwrap()).unwrap();
        symlink(&outside, root.join("2507.19457.json")).unwrap();
        let store = FilesystemAssuranceReportStore::new(&root);
        assert!(matches!(
            store.load("2507.19457"),
            Err(AssuranceStoreError::Storage(_))
        ));
        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_store_rejects_path_traversal() {
        let store = FilesystemAssuranceReportStore::new(temp_root());
        assert!(store.load("../escape").is_err());
        assert!(store.load("nested/id").is_err());
    }
}
