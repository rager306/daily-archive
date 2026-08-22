//! M443: materialization hot-path property-index contract.
//!
//! The ingest/extraction/revision materialization use cases resolve identity
//! through `find_node_by_string_property` on a small set of keys that are
//! **unique by construction** (canonical vids, arxiv ids, source codes).
//! Without a property index each lookup is a linear scan over the label —
//! at full catalog scale (66k nodes) that turned the 589-paper run into a
//! ~2h job. SamyamaGraphStore already implements `create_property_index`
//! with backfill and write tracking; this module is the application-side
//! contract listing exactly which keys must be indexed, so composition
//! roots can ensure them once after store open / snapshot preload.
//!
//! Deliberately excluded: `("Citation", "arxiv_id")` — citation ids repeat
//! across papers, and the indexed path returns the minimum node id while
//! the scan path returns the first match in label order. Indexing it could
//! silently change duplicate resolution semantics; it stays on the scan
//! path until duplicate handling is made explicit.

use da_ports::graph_store::DirectGraphStore;
use std::sync::Arc;

/// Unique identity keys the materialization hot path resolves.
pub const MATERIALIZATION_PROPERTY_INDEXES: &[(&str, &str)] = &[
    ("Paper", "arxiv_id"),
    ("Paper", "vid"),
    ("PaperRevision", "vid"),
    ("Entity", "vid"),
    ("Section", "vid"),
    ("Reference", "vid"),
    ("SourceManifestation", "vid"),
    ("Category", "vid"),
    ("Source", "code"),
];

/// Ensure every hot-path property index exists on the store. Returns the
/// pairs that were (re)created — creation is idempotent and cheap when the
/// index already exists. Failures are per-key and surfaced as `Err` so a
/// composition root fails loudly instead of silently degrading to scans.
pub async fn ensure_materialization_indexes(
    store: &Arc<dyn DirectGraphStore>,
) -> anyhow::Result<Vec<(&'static str, &'static str)>> {
    let mut created = Vec::new();
    for &(label, property) in MATERIALIZATION_PROPERTY_INDEXES {
        store
            .create_property_index(label, property)
            .await
            .map_err(|e| anyhow::anyhow!("create property index {label}.{property} failed: {e}"))?;
        created.push((label, property));
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_list_covers_the_hot_identity_keys() {
        let pairs: Vec<String> = MATERIALIZATION_PROPERTY_INDEXES
            .iter()
            .map(|(l, p)| format!("{l}.{p}"))
            .collect();
        for expected in [
            "Paper.arxiv_id",
            "Paper.vid",
            "PaperRevision.vid",
            "Entity.vid",
            "Section.vid",
            "Reference.vid",
            "SourceManifestation.vid",
            "Category.vid",
            "Source.code",
        ] {
            assert!(pairs.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn citation_arxiv_id_stays_excluded() {
        // Duplicate semantics: indexed lookup returns min node id, scan
        // returns first label-order match. Excluded until duplicates are
        // handled explicitly (see module docs).
        assert!(
            !MATERIALIZATION_PROPERTY_INDEXES
                .iter()
                .any(|(l, p)| *l == "Citation" && *p == "arxiv_id")
        );
    }
}
