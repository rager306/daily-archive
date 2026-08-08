//! Stamp TemporalEdge fields on graph edges according to meta-ontology policy (M327).
//!
//! ADR-046 / M326: `temporal_class: fact` edges get the full 5-field model
//! (`valid_at`, `created_at`, optional `invalid_at` / `expired_at` / `reference_time`).
//! Structural edges get `created_at` only (composition provenance).
//!
//! Policy source: `OntologyRegistry::is_temporal_edge_type` (YAML-first).

use chrono::{DateTime, Utc};
use da_ports::graph_store::DirectGraphStore;
use kg_ontology::registry::OntologyRegistry;
use kg_ontology::temporal::TemporalEdge;

/// Whether the edge wire string is a temporal fact per external meta-ontology.
///
/// Loads project `data/ontology/` when available; unknown types → not fact
/// (safe default: do not invent temporal fields).
pub fn edge_type_is_temporal_fact(edge_type: &str) -> bool {
    match OntologyRegistry::load_project_meta_ontology() {
        Ok(reg) => reg.is_temporal_edge_type(edge_type),
        Err(_) => {
            // Compiled fallback when YAML unavailable (CI without data/ontology).
            matches!(
                edge_type,
                "CITES"
                    | "MENTIONS"
                    | "SUPPORTS"
                    | "SUPERSEDES"
                    | "CAUSED"
                    | "AFFECTS"
                    | "AFFILIATED_WITH"
                    | "PARTICIPATES_IN"
                    | "MEMBER_OF_CLUSTER"
            )
        }
    }
}

/// Open fact edge at `now` (valid, not yet invalidated).
pub fn open_fact_edge_now(now: DateTime<Utc>) -> TemporalEdge {
    TemporalEdge {
        valid_at: now,
        invalid_at: None,
        expired_at: None,
        reference_time: None,
        created_at: now,
    }
}

/// Write temporal properties for an already-created edge.
///
/// - **Fact** types: `valid_at`, `created_at`, and optional fields when present.
/// - **Structural** types: `created_at` only.
pub async fn stamp_edge_temporal_fields(
    store: &dyn DirectGraphStore,
    edge_id: u64,
    edge_type: &str,
    temporal: &TemporalEdge,
) -> anyhow::Result<()> {
    let is_fact = edge_type_is_temporal_fact(edge_type);

    store
        .set_edge_property_string(edge_id, "created_at", &temporal.created_at.to_rfc3339())
        .await?;

    if is_fact {
        store
            .set_edge_property_string(edge_id, "valid_at", &temporal.valid_at.to_rfc3339())
            .await?;
        if let Some(invalid_at) = temporal.invalid_at {
            store
                .set_edge_property_string(edge_id, "invalid_at", &invalid_at.to_rfc3339())
                .await?;
        }
        if let Some(expired_at) = temporal.expired_at {
            store
                .set_edge_property_string(edge_id, "expired_at", &expired_at.to_rfc3339())
                .await?;
        }
        if let Some(reference_time) = temporal.reference_time {
            store
                .set_edge_property_string(edge_id, "reference_time", &reference_time.to_rfc3339())
                .await?;
        }
    }

    tracing::debug!(edge_id, edge_type, is_fact, "Stamped edge temporal fields");
    Ok(())
}

/// Create edge and stamp temporal fields in one step.
pub async fn create_edge_with_temporal(
    store: &dyn DirectGraphStore,
    source: u64,
    target: u64,
    edge_type: &str,
    temporal: &TemporalEdge,
) -> anyhow::Result<u64> {
    let edge_id = store.create_edge(source, target, edge_type).await?;
    stamp_edge_temporal_fields(store, edge_id, edge_type, temporal).await?;
    Ok(edge_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::relation::{bibliographic, structure};

    #[test]
    fn cites_is_temporal_fact() {
        assert!(edge_type_is_temporal_fact(bibliographic::CITES));
        assert!(edge_type_is_temporal_fact("MENTIONS"));
        assert!(edge_type_is_temporal_fact("SUPPORTS"));
        assert!(edge_type_is_temporal_fact("SUPERSEDES"));
    }

    #[test]
    fn has_part_is_structural() {
        assert!(!edge_type_is_temporal_fact(structure::HAS_PART));
        assert!(!edge_type_is_temporal_fact("FROM_SOURCE"));
        assert!(!edge_type_is_temporal_fact("inCategory"));
    }

    #[test]
    fn open_fact_edge_now_is_open() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let e = open_fact_edge_now(now);
        assert_eq!(e.valid_at, now);
        assert_eq!(e.created_at, now);
        assert!(e.invalid_at.is_none());
        assert!(!e.is_expired());
    }
}
