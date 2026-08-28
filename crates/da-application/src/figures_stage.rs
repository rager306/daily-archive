//! Figures stage (M451 phase-2 port #3): analytic matching over
//! INJECTED page observations (memory: cross-surface work lives in
//! da-application over injected observations). The observation cache is
//! produced by the reference exporter (PyMuPDF); the matching, section
//! join, stats and index assembly are pure Rust.

use da_domain::figure_geometry::{
    CaptionRect, FigureRecord, FigureStats, ImageObs, SectionRange, match_figures, stats,
};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

#[derive(Debug, thiserror::Error)]
pub enum FiguresStageError {
    #[error("figures stage: {0}")]
    Store(#[from] SchemaObjectStoreError),
    #[error("figures stage: parsed unreadable for {0}")]
    NoParsed(String),
    #[error("figures stage: observations unreadable: {0}")]
    Observations(String),
}

/// Page observations document (reference exporter output).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PageObservations {
    #[serde(default)]
    pub captions: Vec<CaptionRect>,
    #[serde(default)]
    pub images: Vec<ImageObs>,
    #[serde(default)]
    pub sections: Vec<SectionRange>,
    #[serde(default)]
    pub extracted_files: std::collections::BTreeMap<u64, String>, // xref -> rel path
}

#[derive(Debug, Clone)]
pub struct FiguresStageOutput {
    pub paper_id: String,
    pub records: Vec<FigureRecord>,
    pub index_json: String,
    pub stats: FigureStats,
}

pub fn build_figures_stage(
    store: &dyn SchemaObjectStore,
    paper_id: &str,
    observations_json: &str,
) -> Result<FiguresStageOutput, FiguresStageError> {
    let obs: PageObservations = serde_json::from_str(observations_json)
        .map_err(|e| FiguresStageError::Observations(e.to_string()))?;

    // sections fallback: parsed sections without ranges (text grammars)
    let mut sections = obs.sections.clone();
    if sections.is_empty() {
        let raw = store
            .read_raw(paper_id, "parsed/article.json")?
            .ok_or_else(|| FiguresStageError::NoParsed(paper_id.to_string()))?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| FiguresStageError::NoParsed(format!("{paper_id}: {e}")))?;
        let list = parsed.get("sections").cloned().unwrap_or_default();
        let items: Vec<serde_json::Value> = serde_json::from_value(list)
            .map_err(|e| FiguresStageError::NoParsed(format!("{paper_id}: sections: {e}")))?;
        for (i, s) in items.iter().enumerate() {
            let (ps, pe) = (
                s.get("page_start").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                s.get("page_end").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            );
            if ps == 0 && pe == 0 {
                continue;
            }
            sections.push(SectionRange {
                section_id: s
                    .get("section_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{paper_id}:{i}")),
                page_start: ps,
                page_end: pe,
                title: s
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                is_furniture: false,
            });
        }
    }

    let mut records = match_figures(&obs.captions, &obs.images, &sections);
    // attach extracted binaries by xref (exporter side)
    // match_figures consumed xrefs in order; we re-walk with the same
    // greedy strategy to know which xref each record used
    let used = resolve_used_xrefs(&obs, &records);
    for (r, xref) in records.iter_mut().zip(used) {
        if let Some(rel) = obs.extracted_files.get(&xref) {
            r.image_file = Some(rel.clone());
        }
    }

    let total_images = obs
        .images
        .iter()
        .map(|i| i.xref)
        .collect::<std::collections::HashSet<_>>()
        .len();
    let st = stats(&obs.captions, &records, total_images);
    let index_json = serde_json::to_string_pretty(&records)
        .map_err(|e| FiguresStageError::Observations(format!("serialize: {e}")))?;
    Ok(FiguresStageOutput {
        paper_id: paper_id.to_string(),
        records,
        index_json,
        stats: st,
    })
}

/// Re-run the greedy assignment to recover per-record xref (kept in sync
/// with match_figures' strategy; single source of truth stays there).
fn resolve_used_xrefs(obs: &PageObservations, records: &[FigureRecord]) -> Vec<u64> {
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(records.len());
    for cap in &obs.captions {
        let cap_y = (cap.y0 + cap.y1) / 2.0;
        let best = obs
            .images
            .iter()
            .filter(|im| im.page == cap.page && !used.contains(&im.xref))
            .min_by(|a, b| {
                let da = ((a.bbox[1] + a.bbox[3]) / 2.0 - cap_y).abs();
                let db = ((b.bbox[1] + b.bbox[3]) / 2.0 - cap_y).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        match best {
            Some(im) => {
                used.insert(im.xref);
                out.push(im.xref);
            }
            None => continue,
        }
    }
    out.truncate(records.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct FakeStore(BTreeMap<(String, String), String>);
    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str {
            "article"
        }
        fn norm(&self) -> Result<da_domain::schema_invariants::ObjectNorm, SchemaObjectStoreError> {
            unimplemented!()
        }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> {
            Ok(vec![])
        }
        fn read_object(
            &self,
            _id: &str,
        ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn read_collection(
            &self,
            _id: &str,
            _rel: &str,
        ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> {
            Ok(true)
        }
        fn file_backed_checks(
            &self,
        ) -> Box<dyn da_domain::schema_invariants::FileBackedChecks + '_> {
            unimplemented!()
        }
        fn read_raw(&self, id: &str, rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(self.0.get(&(id.to_string(), rel.to_string())).cloned())
        }
    }

    #[test]
    fn stage_matches_and_attaches_binaries() {
        let obs = serde_json::json!({
            "captions": [{"label": "Figure 1.1", "page": 5, "y0": 200.0, "y1": 210.0}],
            "images": [{"xref": 42, "page": 5, "bbox": [0.0, 300.0, 100.0, 350.0]}],
            "sections": [{"section_id": "s1", "page_start": 1, "page_end": 10, "title": "Ch", "is_furniture": false}],
            "extracted_files": {"42": "figures/assets/fig-1_1.png"}
        });
        let store = FakeStore(BTreeMap::new());
        let out = build_figures_stage(&store, "m", obs.to_string().as_str()).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(
            out.records[0].image_file.as_deref(),
            Some("figures/assets/fig-1_1.png")
        );
        assert_eq!(out.records[0].section_id.as_deref(), Some("s1"));
        assert_eq!(out.stats.figures_indexed, 1);
        assert_eq!(out.stats.with_image_file, 1);
        assert_eq!(out.stats.unassigned_images, 0);
    }
}
