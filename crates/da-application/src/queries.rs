//! Cypher query builders for daily-archive domain types.
//!
//! ADR-041 WARM path: these queries are executed via EmbeddedClient
//! (Cypher with cost-based planner + late materialization).

use da_domain::paper::PaperSchema;
use da_domain::schema::NodeSchemaDef;
use da_ports::graph_store::DirectGraphStore;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PaperLookupError {
    #[error("paper with arxiv_id '{arxiv_id}' was not found in the graph")]
    NotFound { arxiv_id: String },
}

/// Resolve exactly one requested Paper through the storage property-index port.
/// This avoids inventory walks and, critically, never selects an unrelated
/// first Paper node in a multi-paper graph.
pub async fn find_paper_node_id(
    store: &dyn DirectGraphStore,
    arxiv_id: &str,
) -> Result<u64, PaperLookupError> {
    store
        .find_node_by_string_property(PaperSchema.label(), "arxiv_id", arxiv_id)
        .await
        .ok_or_else(|| PaperLookupError::NotFound {
            arxiv_id: arxiv_id.to_owned(),
        })
}

/// Query builders for Paper nodes.
pub struct PaperQueries;

impl PaperQueries {
    /// Find a paper by its SHA256 VID.
    pub fn find_by_vid(vid: &str) -> String {
        format!("MATCH (n:Paper {{vid: \"{}\"}}) RETURN n", vid)
    }

    /// Find a paper by arxiv_id.
    pub fn find_by_arxiv_id(arxiv_id: &str) -> String {
        format!("MATCH (n:Paper {{arxiv_id: \"{}\"}}) RETURN n", arxiv_id)
    }

    /// Count all Paper nodes.
    pub fn count_all() -> String {
        "MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n)".to_string()
    }

    /// Find papers without embeddings (for backfill).
    pub fn without_embedding() -> String {
        "MATCH (n:Paper) WHERE n.retrieval_eligible = true AND n.embedding IS NULL RETURN n.vid, n.arxiv_id".to_string()
    }

    /// Find papers with stale schema_version (for migration).
    pub fn stale_schema(current_version: u32) -> String {
        format!(
            "MATCH (n:Paper) WHERE n.retrieval_eligible = true AND n.schema_version < {} RETURN n.vid, n.schema_version",
            current_version
        )
    }

    /// K-hop citation neighborhood (for agent context, ADR-038 tri-source S_kn).
    pub fn citation_neighborhood(vid: &str, max_hops: usize) -> String {
        format!(
            "MATCH (n:Paper {{vid: \"{}\"}})-[:{}*1..{}]->(cited:Paper) \
             WHERE cited.retrieval_eligible = true \
             RETURN DISTINCT cited.vid, cited.arxiv_id, cited.title LIMIT 100",
            vid,
            da_domain::relation::bibliographic::CITES,
            max_hops
        )
    }

    /// Papers citing a given paper (reverse citations).
    pub fn cited_by(vid: &str) -> String {
        format!(
            "MATCH (citing:Paper)-[:{}]->(n:Paper {{vid: \"{}\"}}) \
             WHERE citing.retrieval_eligible = true \
             RETURN citing.vid, citing.arxiv_id, citing.title",
            da_domain::relation::bibliographic::CITES,
            vid
        )
    }
}

/// Query builders for Entity nodes.
pub struct EntityQueries;

impl EntityQueries {
    /// Find entities by label (e.g. "Method", "Dataset").
    pub fn by_label(label: &str) -> String {
        format!("MATCH (n:{}) RETURN n.vid, n.label, n.confidence", label)
    }

    /// Find entities without evidence (for graph health, ADR-040 §11.4).
    pub fn without_evidence(label: &str) -> String {
        format!(
            "MATCH (n:{}) WHERE NOT (n)-[:HAS_EVIDENCE]->() RETURN n.vid, n.label",
            label
        )
    }

    /// Find orphan nodes (no edges at all, ADR-040 §11.4).
    pub fn orphans() -> String {
        "MATCH (n) WHERE NOT (n)--() RETURN n.vid, labels(n)".to_string()
    }
}

/// Query builders for Relation edges.
pub struct RelationQueries;

impl RelationQueries {
    /// Find all relations of a given type.
    pub fn by_type(rel_type: &str) -> String {
        format!(
            "MATCH (a)-[r:{}]->(b) RETURN a.vid, b.vid, r.confidence",
            rel_type
        )
    }

    /// Find relations from a specific entity.
    pub fn from_entity(vid: &str) -> String {
        format!(
            "MATCH (a {{vid: \"{}\"}})-[r]->(b) RETURN type(r), b.vid, r.confidence",
            vid
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kg_storage::mock::MockGraphStore;

    #[tokio::test]
    async fn exact_paper_lookup_selects_requested_node_in_multi_paper_graph() {
        let store = MockGraphStore::new();
        let first = store.create_node(PaperSchema.label()).await.unwrap();
        store
            .set_node_property_string(first, "arxiv_id", "1111.11111".into())
            .await
            .unwrap();
        let requested = store.create_node(PaperSchema.label()).await.unwrap();
        store
            .set_node_property_string(requested, "arxiv_id", "2507.19457".into())
            .await
            .unwrap();

        assert_eq!(
            find_paper_node_id(&store, "2507.19457").await.unwrap(),
            requested
        );
        assert_ne!(first, requested);
    }

    #[tokio::test]
    async fn exact_paper_lookup_reports_missing_id() {
        let store = MockGraphStore::new();
        let error = find_paper_node_id(&store, "missing").await.unwrap_err();
        assert_eq!(
            error,
            PaperLookupError::NotFound {
                arxiv_id: "missing".into()
            }
        );
    }

    #[test]
    fn test_find_by_vid() {
        let q = PaperQueries::find_by_vid("abc123");
        assert!(q.contains("MATCH (n:Paper {vid: \"abc123\"})"));
    }

    #[test]
    fn test_citation_neighborhood() {
        let q = PaperQueries::citation_neighborhood("vid:abc", 3);
        assert!(q.contains("[:CITES*1..3]"));
        assert!(q.contains("LIMIT 100"));
    }

    #[test]
    fn test_without_evidence() {
        let q = EntityQueries::without_evidence("Method");
        assert!(q.contains("NOT (n)-[:HAS_EVIDENCE]->()"));
    }

    #[test]
    fn test_orphans() {
        let q = EntityQueries::orphans();
        assert!(q.contains("NOT (n)--()"));
    }

    #[test]
    fn test_stale_schema() {
        let q = PaperQueries::stale_schema(2);
        assert!(q.contains("n.schema_version < 2"));
    }

    #[test]
    fn test_find_by_arxiv_id() {
        let q = PaperQueries::find_by_arxiv_id("2507.19457");
        assert!(q.contains("MATCH (n:Paper {arxiv_id: \"2507.19457\"})"));
    }

    #[test]
    fn test_count_all_filters_retrieval_eligible() {
        // D134: retrieval_eligible must be on ALL nodes — count_all must filter.
        let q = PaperQueries::count_all();
        assert!(q.contains("n.retrieval_eligible = true"));
        assert!(q.contains("count(n)"));
    }

    #[test]
    fn test_without_embedding() {
        let q = PaperQueries::without_embedding();
        assert!(q.contains("n.embedding IS NULL"));
        assert!(q.contains("n.retrieval_eligible = true"));
    }

    #[test]
    fn test_cited_by() {
        let q = PaperQueries::cited_by("vid:abc");
        assert!(q.contains("-[:CITES]->"));
        assert!(q.contains("vid: \"vid:abc\""));
        assert!(q.contains("citing.retrieval_eligible = true"));
    }

    #[test]
    fn test_by_label() {
        let q = EntityQueries::by_label("Method");
        assert!(q.contains("MATCH (n:Method)"));
        assert!(q.contains("n.confidence"));
    }

    #[test]
    fn test_relation_by_type() {
        let q = RelationQueries::by_type("CITES");
        assert!(q.contains("-[r:CITES]->"));
        assert!(q.contains("r.confidence"));
    }
}
