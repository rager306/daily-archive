//! Filesystem jsonl store for negative semantic knowledge (M455).
//! One file per paper: `{dir}/{paper_id}.jsonl`. Path owned by CLI/env.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use da_domain::negative_memory::NegativeMemoryRecord;
use da_ports::negative_memory::{NegativeMemoryError, NegativeMemoryPort};

pub struct JsonlNegativeMemory {
    dir: PathBuf,
}

impl JsonlNegativeMemory {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, NegativeMemoryError> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
        Ok(Self { dir })
    }

    fn path_for(&self, paper_id: &str) -> PathBuf {
        self.dir.join(format!("{paper_id}.jsonl"))
    }
}

impl NegativeMemoryPort for JsonlNegativeMemory {
    fn append(&mut self, rec: &NegativeMemoryRecord) -> Result<(), NegativeMemoryError> {
        let path = self.path_for(&rec.paper_id);
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
        let line = serde_json::to_string(rec).map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
        writeln!(f, "{line}").map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
        Ok(())
    }

    fn list_for_paper(&self, paper_id: &str) -> Result<Vec<NegativeMemoryRecord>, NegativeMemoryError> {
        let path = self.path_for(paper_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let f = fs::File::open(&path).map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
        let mut out = Vec::new();
        for line in BufReader::new(f).lines() {
            let line = line.map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let rec: NegativeMemoryRecord =
                serde_json::from_str(&line).map_err(|e| NegativeMemoryError::Io(e.to_string()))?;
            out.push(rec);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::negative_memory::{admit_claim, AdmitKey, record_from_exhausted};

    #[test]
    fn persist_then_admit_denies() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = JsonlNegativeMemory::open(dir.path()).unwrap();
        let rec = record_from_exhausted(
            "n1".into(),
            "2507.19457".into(),
            "figure_analysis".into(),
            Some("schema violation".into()),
            Some("fig-1".into()),
            "2026-08-25T00:00:00Z".into(),
        );
        store.append(&rec).unwrap();
        let listed = store.list_for_paper("2507.19457").unwrap();
        assert_eq!(listed.len(), 1);
        let d = admit_claim(
            AdmitKey {
                paper_id: "2507.19457",
                stage: "figure_analysis",
                pattern: Some("schema violation"),
            },
            &listed,
        );
        assert!(!d.is_allowed());
        assert!(store.list_for_paper("other").unwrap().is_empty());
    }
}
