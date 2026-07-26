//! Schema initialization and versioning for Samyama graph.
//!
//! ADR-040 §11: schema-as-code. This module generates Cypher DDL
//! for creating indexes and initial schema setup.

/// Current schema version (ADR-040 §11.2).
/// Increment when schema changes require migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Schema version type alias for clarity.
pub type SchemaVersion = u32;

/// Schema initializer — generates Cypher for index creation.
pub struct SchemaInitializer;

impl SchemaInitializer {
    /// Cypher to create a vector index on Paper.embedding.
    pub fn create_paper_vector_index(dimensions: usize) -> String {
        format!(
            "CREATE VECTOR INDEX paper_embedding IF NOT EXISTS \
             FOR (n:Paper) ON (n.embedding) \
             OPTIONS {{dimension: {}, metric: 'cosine'}}",
            dimensions
        )
    }

    /// Cypher to create a property index on Paper.vid (for fast lookups).
    pub fn create_paper_vid_index() -> String {
        "CREATE INDEX paper_vid IF NOT EXISTS FOR (n:Paper) ON (n.vid)".to_string()
    }

    /// Cypher to create a property index on Paper.arxiv_id.
    pub fn create_paper_arxiv_id_index() -> String {
        "CREATE INDEX paper_arxiv_id IF NOT EXISTS FOR (n:Paper) ON (n.arxiv_id)".to_string()
    }

    /// All schema initialization Cypher statements in order.
    pub fn all_init_statements(dimensions: usize) -> Vec<String> {
        vec![
            Self::create_paper_vid_index(),
            Self::create_paper_arxiv_id_index(),
            Self::create_paper_vector_index(dimensions),
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
        assert_eq!(stmts.len(), 3);
        assert!(stmts.iter().all(|s| s.contains("CREATE")));
    }
}
