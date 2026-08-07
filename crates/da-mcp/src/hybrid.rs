//! Hybrid search wrapper — bridges ruvector-hybrid RrfHybridIndex with
//! the MCP server's Paper graph data (D194 Wave 2).
//!
//! Builds an in-memory BM25+dense index from Paper nodes at startup.
//! Query: tokenize + embed → RRF fusion → ranked arxiv_id list.

use std::collections::HashMap;

use ruvector_hybrid::{Document, HybridSearch, RrfHybridIndex};

/// A paper entry in the hybrid search index.
#[derive(Debug, Clone)]
pub struct PaperEntry {
    pub arxiv_id: String,
    pub title: String,
}

/// In-memory hybrid search index over Paper nodes.
/// Built once from graph data; queried via `search()`.
pub struct HybridSearchIndex {
    index: Option<RrfHybridIndex>,
    papers: Vec<PaperEntry>,
    id_to_arxiv: HashMap<usize, String>,
}

impl HybridSearchIndex {
    /// Create an empty index (no papers loaded).
    pub fn empty() -> Self {
        Self {
            index: None,
            papers: Vec::new(),
            id_to_arxiv: HashMap::new(),
        }
    }

    /// Build the hybrid index from paper entries + pre-computed embeddings.
    /// Each paper gets a Document with tokenized title and embedding vector.
    pub fn build(papers: Vec<PaperEntry>, embeddings: Vec<Vec<f32>>) -> Self {
        if papers.is_empty() || papers.len() != embeddings.len() {
            return Self::empty();
        }
        let mut docs = Vec::with_capacity(papers.len());
        let mut id_map = HashMap::new();
        for (i, (paper, vec)) in papers.iter().zip(embeddings.iter()).enumerate() {
            let tokens: Vec<String> = paper
                .title
                .to_lowercase()
                .split_whitespace()
                .map(String::from)
                .collect();
            docs.push(Document {
                id: i,
                tokens,
                vector: vec.clone(),
            });
            id_map.insert(i, paper.arxiv_id.clone());
        }
        Self {
            index: Some(RrfHybridIndex::build(&docs)),
            papers,
            id_to_arxiv: id_map,
        }
    }

    /// Search the hybrid index. Returns ranked arxiv_ids via RRF fusion.
    /// Falls back to empty if index not built.
    pub fn search(
        &self,
        query_tokens: &[&str],
        query_vector: &[f32],
        k: usize,
    ) -> Vec<(String, f32)> {
        let Some(ref idx) = self.index else {
            return Vec::new();
        };
        let results = idx.search(query_tokens, query_vector, k);
        results
            .into_iter()
            .filter_map(|r| {
                self.id_to_arxiv
                    .get(&r.id)
                    .map(|aid| (aid.clone(), r.score))
            })
            .collect()
    }

    /// Number of papers in the index.
    pub fn len(&self) -> usize {
        self.papers.len()
    }

    /// Is the index empty?
    pub fn is_empty(&self) -> bool {
        self.papers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_index_empty_returns_nothing() {
        let idx = HybridSearchIndex::empty();
        let results = idx.search(&["test"], &[0.1; 8], 5);
        assert!(results.is_empty(), "empty index must return nothing");
    }

    #[test]
    fn test_hybrid_index_build_and_search() {
        let papers = vec![
            PaperEntry {
                arxiv_id: "2501.001".into(),
                title: "Diffusion Models for Graph Generation".into(),
            },
            PaperEntry {
                arxiv_id: "2501.002".into(),
                title: "Transformer Attention Mechanism".into(),
            },
            PaperEntry {
                arxiv_id: "2501.003".into(),
                title: "Graph Neural Networks for Node Classification".into(),
            },
        ];
        // Mock embeddings: each paper has a distinct 4-dim vector
        let embeddings = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ];
        let idx = HybridSearchIndex::build(papers, embeddings);
        assert_eq!(idx.len(), 3);
        assert!(!idx.is_empty());

        // Search for "graph" — should find papers 0 and 2 (both have "graph" in title)
        let results = idx.search(&["graph"], &[0.5, 0.0, 0.5, 0.0], 3);
        assert!(!results.is_empty(), "hybrid search must return results");
        let ids: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
        // At least one of the graph papers should be in top results
        assert!(
            ids.contains(&"2501.001") || ids.contains(&"2501.003"),
            "graph papers must be found, got: {ids:?}"
        );
    }

    #[test]
    fn test_hybrid_index_mismatched_lengths_returns_empty() {
        let papers = vec![PaperEntry {
            arxiv_id: "1".into(),
            title: "Test".into(),
        }];
        let embeddings: Vec<Vec<f32>> = vec![]; // mismatched
        let idx = HybridSearchIndex::build(papers, embeddings);
        assert!(
            idx.is_empty(),
            "mismatched lengths must produce empty index"
        );
    }
}
