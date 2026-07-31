//! Schema initialization and versioning for Samyama graph.
//!
//! ADR-040 §11 + GRAPH-SCHEMA.md: schema-as-code. This module generates
//! Cypher DDL for creating indexes. The single source of truth is
//! doc/GRAPH-SCHEMA.md; this module mirrors it in code.
//!
//! Run `da schema init` before any data load to create all indexes.

/// Current schema version (ADR-040 §11.2).
/// Increment when schema changes require migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Schema version type alias for clarity.
pub type SchemaVersion = u32;

/// Schema initializer — generates Cypher for index creation.
/// Mirrors doc/GRAPH-SCHEMA.md §Indexes.
pub struct SchemaInitializer;

impl SchemaInitializer {
    // ─── Paper indexes ───

    /// Cypher to create a unique property index on Paper.vid.
    pub fn create_paper_vid_index() -> String {
        "CREATE INDEX paper_vid IF NOT EXISTS FOR (n:Paper) ON (n.vid)".to_string()
    }

    /// Cypher to create a property index on Paper.arxiv_id.
    pub fn create_paper_arxiv_id_index() -> String {
        "CREATE INDEX paper_arxiv_id IF NOT EXISTS FOR (n:Paper) ON (n.arxiv_id)".to_string()
    }

    /// Cypher to create a vector index on Paper.embedding.
    pub fn create_paper_vector_index(dimensions: usize) -> String {
        format!(
            "CREATE VECTOR INDEX paper_embedding IF NOT EXISTS \
             FOR (n:Paper) ON (n.embedding) \
             OPTIONS {{dimension: {}, metric: 'cosine'}}",
            dimensions
        )
    }

    // ─── Citation indexes ───

    /// Cypher to create a unique property index on Citation.vid.
    pub fn create_citation_vid_index() -> String {
        "CREATE INDEX citation_vid IF NOT EXISTS FOR (n:Citation) ON (n.vid)".to_string()
    }

    /// Cypher to create a property index on Citation.arxiv_id.
    pub fn create_citation_arxiv_id_index() -> String {
        "CREATE INDEX citation_arxiv_id IF NOT EXISTS FOR (n:Citation) ON (n.arxiv_id)".to_string()
    }

    // ─── Entity indexes ───

    /// Cypher to create a unique property index on Entity.vid.
    pub fn create_entity_vid_index() -> String {
        "CREATE INDEX entity_vid IF NOT EXISTS FOR (n:Entity) ON (n.vid)".to_string()
    }

    /// Cypher to create a property index on Entity.entity_type.
    pub fn create_entity_type_index() -> String {
        "CREATE INDEX entity_type IF NOT EXISTS FOR (n:Entity) ON (n.entity_type)".to_string()
    }

    // ─── Section indexes ───

    pub fn create_section_vid_index() -> String {
        "CREATE INDEX section_vid IF NOT EXISTS FOR (n:Section) ON (n.vid)".to_string()
    }

    pub fn create_section_paper_index() -> String {
        "CREATE INDEX section_paper IF NOT EXISTS FOR (n:Section) ON (n.paper_id)".to_string()
    }

    // ─── Keyword indexes ───

    pub fn create_keyword_vid_index() -> String {
        "CREATE INDEX keyword_vid IF NOT EXISTS FOR (n:Keyword) ON (n.vid)".to_string()
    }

    pub fn create_keyword_text_index() -> String {
        "CREATE INDEX keyword_text IF NOT EXISTS FOR (n:Keyword) ON (n.keyword)".to_string()
    }

    // ─── Topic indexes ───

    pub fn create_topic_vid_index() -> String {
        "CREATE INDEX topic_vid IF NOT EXISTS FOR (n:Topic) ON (n.vid)".to_string()
    }

    pub fn create_topic_label_index() -> String {
        "CREATE INDEX topic_label IF NOT EXISTS FOR (n:Topic) ON (n.label)".to_string()
    }

    // ─── Category indexes ───

    pub fn create_category_vid_index() -> String {
        "CREATE INDEX category_vid IF NOT EXISTS FOR (n:Category) ON (n.vid)".to_string()
    }

    pub fn create_category_code_index() -> String {
        "CREATE INDEX category_code IF NOT EXISTS FOR (n:Category) ON (n.code)".to_string()
    }

    // --- Source (Layer 0: ONTOLOGY-DESIGN) ---

    pub fn create_source_vid_index() -> String {
        "CREATE INDEX source_vid IF NOT EXISTS FOR (n:Source) ON (n.vid)".to_string()
    }

    pub fn create_source_code_index() -> String {
        "CREATE INDEX source_code IF NOT EXISTS FOR (n:Source) ON (n.code)".to_string()
    }

    // --- ConceptCluster (Layer 6: ONTOLOGY-DESIGN Hypergraph) ---

    pub fn create_cluster_vid_index() -> String {
        "CREATE INDEX cluster_vid IF NOT EXISTS FOR (n:ConceptCluster) ON (n.vid)".to_string()
    }

    pub fn create_cluster_type_index() -> String {
        "CREATE INDEX cluster_type IF NOT EXISTS FOR (n:ConceptCluster) ON (n.cluster_type)"
            .to_string()
    }

    // --- Reference (bibliography entries, Layer 1) ---

    pub fn create_reference_vid_index() -> String {
        "CREATE INDEX reference_vid IF NOT EXISTS FOR (n:Reference) ON (n.vid)".to_string()
    }

    // --- Author (Layer 1 metadata) ---

    pub fn create_author_vid_index() -> String {
        "CREATE INDEX author_vid IF NOT EXISTS FOR (n:Author) ON (n.vid)".to_string()
    }

    // --- Institution (Layer 1 metadata) ---

    pub fn create_institution_openalex_id_index() -> String {
        "CREATE INDEX institution_openalex_id IF NOT EXISTS FOR (n:Institution) ON (n.openalex_id)"
            .to_string()
    }

    // --- EvidenceBundle (Layer 6 evidence) ---

    pub fn create_evidence_bundle_vid_index() -> String {
        "CREATE INDEX evidence_bundle_vid IF NOT EXISTS FOR (n:EvidenceBundle) ON (n.vid)"
            .to_string()
    }

    // --- Claim (Layer 6 proposition) ---

    pub fn create_claim_vid_index() -> String {
        "CREATE INDEX claim_vid IF NOT EXISTS FOR (n:Claim) ON (n.vid)".to_string()
    }

    // --- SchedulerTask (Layer 1 operational state) ---

    pub fn create_schedulertask_arxiv_id_index() -> String {
        "CREATE INDEX schedulertask_arxiv_id IF NOT EXISTS FOR (n:SchedulerTask) ON (n.arxiv_id)"
            .to_string()
    }

    // --- Entity embedding (Phase 3 GNN readiness) ---

    pub fn create_entity_vector_index(dimensions: usize) -> String {
        format!(
            "CREATE VECTOR INDEX entity_embedding IF NOT EXISTS FOR (n:Entity) ON (n.embedding) OPTIONS {{indexConfig: {{`vector.dimensions`: {}, `vector.similarity_function`: 'cosine'}}}}",
            dimensions
        )
    }

    /// All schema initialization Cypher statements in order (GRAPH-SCHEMA.md).
    pub fn all_init_statements(dimensions: usize) -> Vec<String> {
        vec![
            // Paper
            Self::create_paper_vid_index(),
            Self::create_paper_arxiv_id_index(),
            Self::create_paper_vector_index(dimensions),
            // Citation
            Self::create_citation_vid_index(),
            Self::create_citation_arxiv_id_index(),
            // Entity
            Self::create_entity_vid_index(),
            Self::create_entity_type_index(),
            // Section
            Self::create_section_vid_index(),
            Self::create_section_paper_index(),
            // Keyword
            Self::create_keyword_vid_index(),
            Self::create_keyword_text_index(),
            // Topic
            Self::create_topic_vid_index(),
            Self::create_topic_label_index(),
            // Category
            Self::create_category_vid_index(),
            Self::create_category_code_index(),
            // Source (Layer 0)
            Self::create_source_vid_index(),
            Self::create_source_code_index(),
            // ConceptCluster (Layer 6 Hypergraph)
            Self::create_cluster_vid_index(),
            Self::create_cluster_type_index(),
            // Reference (Layer 1 bibliography)
            Self::create_reference_vid_index(),
            // Author (Layer 1 metadata)
            Self::create_author_vid_index(),
            // Institution (Layer 1 metadata)
            Self::create_institution_openalex_id_index(),
            // EvidenceBundle (Layer 6 evidence)
            Self::create_evidence_bundle_vid_index(),
            // Claim (Layer 6 proposition)
            Self::create_claim_vid_index(),
            // SchedulerTask (Layer 1 operational state)
            Self::create_schedulertask_arxiv_id_index(),
            // Entity embedding (Phase 3 GNN readiness)
            Self::create_entity_vector_index(dimensions),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_schema_version() {
        assert_eq!(CURRENT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn test_vector_index_cypher() {
        let q = SchemaInitializer::create_paper_vector_index(1024);
        assert!(q.contains("dimension: 1024"));
        assert!(q.contains("cosine"));
    }

    #[test]
    fn test_all_init_statements() {
        let stmts = SchemaInitializer::all_init_statements(1024);
        // 3 Paper + 2 Citation + 2 Entity + 2 Section + 2 Keyword + 2 Topic + 2 Category
        // + 2 Source + 2 ConceptCluster + 1 Reference + 1 Author + 1 Institution
        // + 1 SchedulerTask + 1 Entity vector = 24
        assert_eq!(stmts.len(), 26);
        assert!(stmts.iter().all(|s| s.contains("CREATE")));
    }

    #[test]
    fn test_citation_indexes() {
        assert!(SchemaInitializer::create_citation_vid_index().contains("Citation"));
        assert!(SchemaInitializer::create_citation_arxiv_id_index().contains("arxiv_id"));
    }

    #[test]
    fn test_entity_indexes() {
        assert!(SchemaInitializer::create_entity_vid_index().contains("Entity"));
        assert!(SchemaInitializer::create_entity_type_index().contains("entity_type"));
    }
}
