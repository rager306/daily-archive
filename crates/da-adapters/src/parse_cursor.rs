//! Optional redb-backed parse-batch accelerator (D295, M464-z84in2 S06/T04).
//!
//! Compiled ONLY under the `redb` feature. The accelerator stores two
//! rebuildable lookups keyed by the batch ordered input fingerprint:
//!
//! - the resume **cursor** (next unprocessed ordinal of the durably
//!   acknowledged contiguous terminal prefix), and
//! - the **manifest index** (ordinal → logical paper id for one admitted
//!   batch input).
//!
//! Non-authority contract (D295 / ADR-040): deleting or corrupting this
//! file never destroys unique state. The canonical terminal evidence is
//! the versioned `da parse-batch` result report plus the
//! `FilePipelineCache` payload/manifest records — a fresh run (cache-hit
//! replay) fully rebuilds the accelerator. Error strings are static and
//! closed: no paths, no URLs, no payload material.

use std::path::Path;
use std::sync::Arc;

use da_parse::{BatchCheckpoint, BatchCursor, ParseBatchSinkError};

/// redb table: ordered input fingerprint (64-hex) → next unprocessed ordinal.
const CURSOR_TABLE: redb::TableDefinition<'_, &str, u32> =
    redb::TableDefinition::new("parse_batch_cursor_v1");

/// redb table: ordered input fingerprint → JSON `[(ordinal, paper_id)]`.
const MANIFEST_TABLE: redb::TableDefinition<'_, &str, &str> =
    redb::TableDefinition::new("parse_batch_manifest_index_v1");

/// Closed accelerator failure vocabulary. Static descriptions only — the
/// file location lives with the operator who configured it, never here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCursorAcceleratorError {
    /// The redb file could not be opened or created (IO, lock, or — most
    /// commonly — a corrupt/unreadable header). Deleting the file is safe:
    /// the accelerator is rebuilt from canonical evidence on the next run.
    OpenFailed,
    /// A read lookup failed inside a healthy database.
    ReadFailed,
    /// A durable write (cursor or manifest index) failed; the accelerator
    /// stays non-authoritative, but an enabled run fails closed.
    WriteFailed,
    /// The stored manifest index is not decodable (accelerator drift).
    CorruptIndex,
}

impl std::fmt::Display for ParseCursorAcceleratorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFailed => write!(
                formatter,
                "redb parse cursor accelerator cannot be opened (delete the accelerator file to rebuild it; it is non-authoritative)"
            ),
            Self::ReadFailed => write!(formatter, "redb parse cursor accelerator read failed"),
            Self::WriteFailed => write!(formatter, "redb parse cursor accelerator write failed"),
            Self::CorruptIndex => {
                write!(formatter, "redb parse manifest index is corrupt")
            }
        }
    }
}

impl std::error::Error for ParseCursorAcceleratorError {}

/// The rebuildable redb accelerator over cursor + manifest-index lookups.
#[derive(Debug)]
pub struct ParseCursorAccelerator {
    db: redb::Database,
}

impl ParseCursorAccelerator {
    /// Open (creating if absent) the accelerator database at `path`. The
    /// parent directory is created on demand.
    pub fn open(path: &Path) -> Result<Self, ParseCursorAcceleratorError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| ParseCursorAcceleratorError::OpenFailed)?;
        }
        let db =
            redb::Database::create(path).map_err(|_| ParseCursorAcceleratorError::OpenFailed)?;
        Ok(Self { db })
    }

    /// Durably record the cursor for one ordered input fingerprint.
    pub fn save_cursor(
        &self,
        fingerprint: &str,
        next_ordinal: u32,
    ) -> Result<(), ParseCursorAcceleratorError> {
        let write = self
            .db
            .begin_write()
            .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
        let result = (|| -> Result<(), ParseCursorAcceleratorError> {
            let mut table = write
                .open_table(CURSOR_TABLE)
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
            let _ = table
                .insert(fingerprint, next_ordinal)
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
            Ok(())
        })();
        match result {
            Ok(()) => write
                .commit()
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed),
            Err(error) => Err(error),
        }
    }

    /// Look up the stored cursor ordinal for `fingerprint` (a missing row
    /// is a normal `Ok(None)` — the accelerator is an index, not truth).
    pub fn load_cursor(
        &self,
        fingerprint: &str,
    ) -> Result<Option<u32>, ParseCursorAcceleratorError> {
        let read = self
            .db
            .begin_read()
            .map_err(|_| ParseCursorAcceleratorError::ReadFailed)?;
        let table = match read.open_table(CURSOR_TABLE) {
            Ok(table) => table,
            // Table never created yet: an honest miss.
            Err(_) => return Ok(None),
        };
        match table.get(fingerprint) {
            Ok(value) => Ok(value.map(|guard| guard.value())),
            Err(_) => Err(ParseCursorAcceleratorError::ReadFailed),
        }
    }

    /// Index one admitted manifest: ordinal → logical paper id rows for
    /// `fingerprint` (rebuildable; the report stays canonical).
    pub fn index_manifest(
        &self,
        fingerprint: &str,
        entries: &[(u32, String)],
    ) -> Result<(), ParseCursorAcceleratorError> {
        let json = serde_json::to_string(entries)
            .map_err(|_| ParseCursorAcceleratorError::CorruptIndex)?;
        let write = self
            .db
            .begin_write()
            .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
        let result = (|| -> Result<(), ParseCursorAcceleratorError> {
            let mut table = write
                .open_table(MANIFEST_TABLE)
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
            let _ = table
                .insert(fingerprint, json.as_str())
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
            Ok(())
        })();
        match result {
            Ok(()) => write
                .commit()
                .map_err(|_| ParseCursorAcceleratorError::WriteFailed),
            Err(error) => Err(error),
        }
    }

    /// Read the indexed manifest rows for `fingerprint`.
    pub fn manifest_entries(
        &self,
        fingerprint: &str,
    ) -> Result<Option<Vec<(u32, String)>>, ParseCursorAcceleratorError> {
        let read = self
            .db
            .begin_read()
            .map_err(|_| ParseCursorAcceleratorError::ReadFailed)?;
        let table = match read.open_table(MANIFEST_TABLE) {
            Ok(table) => table,
            Err(_) => return Ok(None),
        };
        let Some(value) = table
            .get(fingerprint)
            .map_err(|_| ParseCursorAcceleratorError::ReadFailed)?
        else {
            return Ok(None);
        };
        serde_json::from_str(value.value())
            .map(Some)
            .map_err(|_| ParseCursorAcceleratorError::CorruptIndex)
    }

    /// Drop both accelerator rows for `fingerprint` (rebuild hygiene; the
    /// canonical report and cache records are untouched).
    pub fn forget_batch(&self, fingerprint: &str) -> Result<(), ParseCursorAcceleratorError> {
        let write = self
            .db
            .begin_write()
            .map_err(|_| ParseCursorAcceleratorError::WriteFailed)?;
        if let Ok(mut table) = write.open_table(CURSOR_TABLE) {
            let _ = table.remove(fingerprint);
        }
        if let Ok(mut table) = write.open_table(MANIFEST_TABLE) {
            let _ = table.remove(fingerprint);
        }
        write
            .commit()
            .map_err(|_| ParseCursorAcceleratorError::WriteFailed)
    }
}

/// [`BatchCheckpoint`] over the accelerator: executor cursor advancement is
/// mirrored into redb (a lookup accelerator, not the canonical record — the
/// atomic result report remains the canonical cursor evidence).
#[derive(Debug, Clone)]
pub struct RedbCursorCheckpoint {
    accelerator: Arc<ParseCursorAccelerator>,
}

impl RedbCursorCheckpoint {
    /// Checkpoint over a shared accelerator.
    pub fn new(accelerator: Arc<ParseCursorAccelerator>) -> Self {
        Self { accelerator }
    }

    /// The shared accelerator (diagnostics/tests).
    pub fn accelerator(&self) -> &ParseCursorAccelerator {
        &self.accelerator
    }
}

#[async_trait::async_trait]
impl BatchCheckpoint for RedbCursorCheckpoint {
    async fn save(&self, cursor: &BatchCursor) -> Result<(), ParseBatchSinkError> {
        self.accelerator
            .save_cursor(cursor.input_fingerprint(), cursor.next_ordinal())
            .map_err(|error| ParseBatchSinkError(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("parse-cursor-{name}-"))
            .tempdir()
            .expect("tempdir")
    }

    #[test]
    fn cursor_roundtrip_persists_across_reopen() {
        let root = temp_root("roundtrip");
        let path = root.path().join("nested").join("accel.redb");
        {
            let accelerator = ParseCursorAccelerator::open(&path).expect("open");
            assert_eq!(accelerator.load_cursor("fp").expect("load"), None);
            accelerator.save_cursor("fp", 3).expect("save");
            accelerator
                .index_manifest("fp", &[(0, "doc-a".into()), (1, "doc-b".into())])
                .expect("index");
        }
        let reopened = ParseCursorAccelerator::open(&path).expect("reopen");
        assert_eq!(reopened.load_cursor("fp").expect("load"), Some(3));
        assert_eq!(
            reopened.manifest_entries("fp").expect("manifest"),
            Some(vec![(0, "doc-a".into()), (1, "doc-b".into())])
        );
        assert_eq!(reopened.load_cursor("other").expect("load"), None);
    }

    #[test]
    fn cursor_advance_overwrites_row() {
        let root = temp_root("advance");
        let path = root.path().join("accel.redb");
        let accelerator = ParseCursorAccelerator::open(&path).expect("open");
        accelerator.save_cursor("fp", 1).expect("save");
        accelerator.save_cursor("fp", 2).expect("save");
        assert_eq!(accelerator.load_cursor("fp").expect("load"), Some(2));
    }

    #[test]
    fn forget_batch_drops_only_that_fingerprint() {
        let root = temp_root("forget");
        let path = root.path().join("accel.redb");
        let accelerator = ParseCursorAccelerator::open(&path).expect("open");
        accelerator.save_cursor("fp-a", 1).expect("save a");
        accelerator.save_cursor("fp-b", 2).expect("save b");
        accelerator.forget_batch("fp-a").expect("forget");
        assert_eq!(accelerator.load_cursor("fp-a").expect("load"), None);
        assert_eq!(accelerator.load_cursor("fp-b").expect("load"), Some(2));
    }

    #[test]
    fn corrupt_file_fails_closed_and_delete_enables_rebuild() {
        let root = temp_root("corrupt");
        let path = root.path().join("accel.redb");
        {
            let accelerator = ParseCursorAccelerator::open(&path).expect("open");
            accelerator.save_cursor("fp", 1).expect("save");
        }
        // Corrupt: overwrite with garbage (redb checks its magic header).
        std::fs::write(&path, b"definitely not a redb database").expect("write garbage");
        let corrupted = ParseCursorAccelerator::open(&path);
        assert_eq!(
            corrupted.expect_err("corrupt file must fail closed"),
            ParseCursorAcceleratorError::OpenFailed
        );
        // Rebuild: deletion is safe (non-authoritative) — a fresh open works
        // and the row is restored by the next durable save.
        std::fs::remove_file(&path).expect("delete accelerator");
        let rebuilt = ParseCursorAccelerator::open(&path).expect("reopen");
        assert_eq!(rebuilt.load_cursor("fp").expect("load"), None);
        rebuilt.save_cursor("fp", 4).expect("rebuild save");
        assert_eq!(rebuilt.load_cursor("fp").expect("load"), Some(4));
    }

    #[tokio::test]
    async fn checkpoint_seam_mirrors_executor_cursor() {
        let root = temp_root("checkpoint");
        let path = root.path().join("accel.redb");
        let accelerator = Arc::new(ParseCursorAccelerator::open(&path).expect("open"));
        let checkpoint = RedbCursorCheckpoint::new(accelerator.clone());
        checkpoint
            .save(&BatchCursor::at("fp", 5))
            .await
            .expect("checkpoint save");
        assert_eq!(accelerator.load_cursor("fp").expect("load"), Some(5));
    }
}
