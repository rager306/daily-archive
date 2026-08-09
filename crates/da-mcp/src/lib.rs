//! MCP server for daily-archive knowledge base external access (ADR-054).
//!
//! Read-only MCP server over Samyama graph store (ADR-054). Exposes:
//! - Tools: get_schema, get_graph_stats, read_cypher, get_paper, get_sections,
//!   get_references, search_papers, get_entities, validate_node, vector_search,
//!   get_summary
//! - Resources: 6 ontology YAML descriptors (list_mcp_resources)
//! - Prompts: 6 agent guidance templates (list_mcp_prompts)
//!
//! D127-safe: no write tools, no import unlock, query_readonly enforced.

use std::sync::Arc;

use kg_storage::DirectGraphStore;
use rmcp::handler::server::wrapper::Parameters;

pub mod hybrid;
pub mod tools;

/// A browsable MCP resource (ADR-054 MCP-4). Thin descriptor for external discovery.
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// A reusable MCP prompt template (ADR-054 MCP-4).
#[derive(Debug, Clone)]
pub struct McpPrompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<McpPromptArgument>,
}

/// A named argument for an MCP prompt template.
#[derive(Debug, Clone)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// Configuration for the MCP server.
#[derive(Debug, Clone, Default)]
pub struct McpServerConfig {
    /// Snapshot file path (`.sgsnap`). When set, the server loads the graph
    /// into an in-process Samyama engine.
    pub snapshot_path: Option<String>,
}

/// The MCP server handler — read-only tools over the graph store.
pub struct DailyArchiveMcpServer {
    pub config: McpServerConfig,
    pub graph_store: Arc<dyn DirectGraphStore>,
    pub embedder: Arc<dyn kg_embeddings::Embedder + Send + Sync>,
}

impl DailyArchiveMcpServer {
    /// Create a new server with the given graph store and default config.
    /// Uses FdApiEmbedder (1024d) when FD_API_KEY is set, falls back to MockEmbedder.
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        let embedder: Arc<dyn kg_embeddings::Embedder + Send + Sync> =
            Arc::new(kg_embeddings::FdApiEmbedder::from_env());
        Self {
            config: McpServerConfig::default(),
            graph_store,
            embedder,
        }
    }

    /// Create a new server with snapshot mode.
    pub fn with_snapshot(
        graph_store: Arc<dyn DirectGraphStore>,
        snapshot_path: impl Into<String>,
    ) -> Self {
        let embedder: Arc<dyn kg_embeddings::Embedder + Send + Sync> =
            Arc::new(kg_embeddings::FdApiEmbedder::from_env());
        Self {
            config: McpServerConfig {
                snapshot_path: Some(snapshot_path.into()),
            },
            graph_store,
            embedder,
        }
    }

    /// Wave 1 MCP-4 (D194): list ontology YAML + schema resources for external browsing.
    /// Returns static resource descriptors; content is read via `read_mcp_resource`.
    pub fn list_mcp_resources(&self) -> Vec<McpResource> {
        vec![
            McpResource {
                uri: "graph://ontology/node_schemas".into(),
                name: "Node Schemas".into(),
                description: Some("ADR-050 node schema definitions (31 labels)".into()),
                mime_type: Some("text/yaml".into()),
            },
            McpResource {
                uri: "graph://ontology/edge_contracts".into(),
                name: "Edge Contracts".into(),
                description: Some(
                    "ADR-053 edge type contracts (navigation, CITES, MENTIONS, ...)".into(),
                ),
                mime_type: Some("text/yaml".into()),
            },
            McpResource {
                uri: "graph://ontology/decision_categories".into(),
                name: "Decision Categories".into(),
                description: Some("ADR-038 decision classification categories".into()),
                mime_type: Some("text/yaml".into()),
            },
            McpResource {
                uri: "graph://ontology/node_vocabulary".into(),
                name: "Node Vocabulary".into(),
                description: Some("Controlled vocabularies for typed fields".into()),
                mime_type: Some("text/yaml".into()),
            },
            McpResource {
                uri: "graph://schema".into(),
                name: "Schema Summary".into(),
                description: Some("Quick summary: node labels + edge types + counts".into()),
                mime_type: Some("text/plain".into()),
            },
            McpResource {
                uri: "graph://stats".into(),
                name: "Graph Statistics".into(),
                description: Some("Current node/edge counts, label distribution".into()),
                mime_type: Some("application/json".into()),
            },
        ]
    }

    /// Wave 1 MCP-4 (D194): read a resource by URI. Returns YAML/text content.
    pub fn read_mcp_resource(&self, uri: &str) -> Option<String> {
        match uri {
            "graph://ontology/node_schemas" => Some(
                include_str!("../../../data/ontology/node_schemas.yaml").to_string(),
            ),
            "graph://ontology/edge_contracts" => Some(
                include_str!("../../../data/ontology/edge_contracts.yaml").to_string(),
            ),
            "graph://ontology/decision_categories" => Some(
                include_str!("../../../data/ontology/decision_categories.yaml").to_string(),
            ),
            "graph://ontology/node_vocabulary" => {
                Some(include_str!("../../../data/node_vocabulary.yaml").to_string())
            }
            "graph://schema" => {
                let labels = data_ontology_labels();
                Some(format!("Node labels ({})\n{}", labels.len(), labels.join(", ")))
            }
            "graph://stats" => {
                Some("{\"resource\": \"graph://stats\", \"note\": \"Use get_graph_stats tool for live counts\"}".into())
            }
            _ => None,
        }
    }

    /// Wave 1 MCP-4 (D194): list prompt templates for agent guidance.
    /// Returns reusable templates that external clients fill with arguments.
    pub fn list_mcp_prompts(&self) -> Vec<McpPrompt> {
        vec![
            McpPrompt {
                name: "literature_review".into(),
                description: "Find papers on a topic, summarize key contributions, and identify research gaps.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "topic".into(),
                        description: "Research topic or question (e.g. 'graph neural networks for retrieval')".into(),
                        required: true,
                    },
                ],
            },
            McpPrompt {
                name: "critical_reading".into(),
                description: "Deep-dive analysis of a specific paper: methodology, claims, evidence quality, limitations.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "arxiv_id".into(),
                        description: "arXiv ID of the paper (e.g. '2507.19457')".into(),
                        required: true,
                    },
                ],
            },
            McpPrompt {
                name: "reproducibility_check".into(),
                description: "Check if a paper's methods, datasets, and code are sufficiently documented for reproduction.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "arxiv_id".into(),
                        description: "arXiv ID of the paper".into(),
                        required: true,
                    },
                ],
            },
            McpPrompt {
                name: "citation_lineage".into(),
                description: "Trace citation ancestry and descendants of a paper through the CITES graph.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "arxiv_id".into(),
                        description: "arXiv ID of the paper".into(),
                        required: true,
                    },
                ],
            },
            McpPrompt {
                name: "compare_methods".into(),
                description: "Compare methods, datasets, and baselines across papers on a shared topic.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "topic".into(),
                        description: "Topic to compare methods on".into(),
                        required: true,
                    },
                ],
            },
            McpPrompt {
                name: "generate_cypher".into(),
                description: "Convert a natural-language question into a Cypher query for the knowledge graph.".into(),
                arguments: vec![
                    McpPromptArgument {
                        name: "question".into(),
                        description: "Natural-language question (e.g. 'Which papers cite transformer architecture?')".into(),
                        required: true,
                    },
                ],
            },
        ]
    }

    /// Wave 1 MCP-4 (D194): get a filled prompt message for a named template.
    /// Returns the rendered prompt text that the client sends to its LLM.
    pub fn get_mcp_prompt(
        &self,
        name: &str,
        args: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        match name {
            "literature_review" => {
                let topic = args.get("topic").map(|s| s.as_str()).unwrap_or("<topic>");
                Some(format!(
                    "You are a research assistant. Search the knowledge graph for papers about: {topic}. \\\n                    Use search_papers(\"{topic}\") and vector_search(\"{topic}\", k=10) to find relevant papers. \\\n                    Then use get_paper and get_sections to read key contributions. \\\n                    Summarize: 1) key findings, 2) methods used, 3) datasets, 4) research gaps. \\\n                    Cite each finding with the paper's arxiv_id."
                ))
            }
            "critical_reading" => {
                let id = args
                    .get("arxiv_id")
                    .map(|s| s.as_str())
                    .unwrap_or("<arxiv_id>");
                Some(format!(
                    "You are a critical reader. Analyze paper {id} in depth. \\\n                    Use get_paper(\"{id}\") for metadata, get_sections(\"{id}\") for structure, \\\n                    get_references(\"{id}\", \"backward\") for citations. \\\n                    Evaluate: 1) claim clarity, 2) methodology rigor, 3) evidence quality, \\\n                    4) limitations, 5) reproducibility. Rate each 1-5 with justification."
                ))
            }
            "reproducibility_check" => {
                let id = args
                    .get("arxiv_id")
                    .map(|s| s.as_str())
                    .unwrap_or("<arxiv_id>");
                Some(format!(
                    "You are a reproducibility auditor. Check paper {id} for: \\\n                    1) Are datasets named and accessible? 2) Are hyperparameters listed? \\\n                    3) Is code/data linked? 4) Are evaluation metrics clearly defined? \\\n                    5) Are baselines reproducible? Use get_entities(\"{id}\") to check typed entities."
                ))
            }
            "citation_lineage" => {
                let id = args
                    .get("arxiv_id")
                    .map(|s| s.as_str())
                    .unwrap_or("<arxiv_id>");
                Some(format!(
                    "You are a citation analyst. Trace the citation graph for paper {id}. \\\n                    Use get_references(\"{id}\", \"backward\") for papers it cites, \\\n                    get_references(\"{id}\", \"forward\") for papers citing it. \\\n                    Identify: 1) foundational ancestors, 2) key descendants, 3) citation clusters."
                ))
            }
            "compare_methods" => {
                let topic = args.get("topic").map(|s| s.as_str()).unwrap_or("<topic>");
                Some(format!(
                    "You are a methods comparator. Find papers on: {topic}. \\\n                    Use search_papers(\"{topic}\") then get_entities for each to extract Method entities. \\\n                    Build a comparison table: method name | dataset | metric | baseline | result."
                ))
            }
            "generate_cypher" => {
                let q = args
                    .get("question")
                    .map(|s| s.as_str())
                    .unwrap_or("<question>");
                Some(format!(
                    "You are a Cypher query generator. Convert this question to a read-only Cypher query: \\\n                    \"{q}\" \\\n                    Schema: Paper(arxiv_id, title, abstract), Section(path_id, title, text), Entity(vid, type). \\\n                    Edges: CITES, MENTIONS, hasPart, next_sibling, first_child, parent_section. \\\n                    Only MATCH/RETURN/WHERE/ORDER BY/LIMIT. No CREATE/SET/DELETE/MERGE."
                ))
            }
            _ => None,
        }
    }
}

/// List node labels from the embedded authoritative ontology registry.
fn data_ontology_labels() -> Vec<String> {
    let registry = kg_ontology::OntologyRegistry::load_from_str(
        include_str!("../../../data/ontology/node_schemas.yaml"),
        "embedded:data/ontology/node_schemas.yaml",
    )
    .expect("embedded node_schemas.yaml must pass ontology registry validation");
    let mut labels = registry.labels_owned();
    labels.sort();
    labels
}

/// Wave 1 tools: get_schema + get_graph_stats. Read-only by construction.
#[rmcp::tool_router(server_handler)]
impl DailyArchiveMcpServer {
    /// Get the daily-archive knowledge graph schema.
    #[rmcp::tool(
        description = "Get the daily-archive knowledge graph schema: node labels + edge types from YAML ontology (ADR-050). Optional: include node/edge counts."
    )]
    async fn get_schema(
        &self,
        parameters: Parameters<tools::GetSchemaParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_schema(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| format!("get_schema failed: {e}"))?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get daily-archive knowledge graph statistics.
    #[rmcp::tool(
        description = "Get daily-archive knowledge graph statistics: total node and edge counts, plus per-label node distribution. Snapshot path if snapshot mode."
    )]
    async fn get_graph_stats(&self) -> Result<String, String> {
        let out = tools::get_graph_stats(
            self.graph_store.as_ref(),
            self.config.snapshot_path.as_deref(),
        )
        .await
        .map_err(|e| format!("get_graph_stats failed: {e}"))?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Run a read-only Cypher query against the knowledge graph.
    #[rmcp::tool(
        description = "Run a read-only Cypher query against the daily-archive knowledge graph. Rejects destructive keywords (CREATE/SET/DELETE/MERGE). D127-safe read-only. Use Samyama Cypher syntax; LIMIT results for large queries."
    )]
    async fn read_cypher(
        &self,
        parameters: Parameters<tools::ReadCypherParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::read_cypher(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get a paper's basic metadata by arXiv id.
    #[rmcp::tool(
        description = "Get a paper node and its basic metadata (title, publication date, cited-by count, OpenAlex id) by arXiv id."
    )]
    async fn get_paper(
        &self,
        parameters: Parameters<tools::GetPaperParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_paper(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get a paper's section tree (title, path_id, level) by arXiv id.
    #[rmcp::tool(
        description = "Get the section navigation tree for a paper by arXiv id: section titles, path_id, and level in document order."
    )]
    async fn get_sections(
        &self,
        parameters: Parameters<tools::GetSectionsParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_sections(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get CITES references for a paper (backward = papers it cites, forward =
    /// papers that cite it, both = default).
    #[rmcp::tool(
        description = "Get CITES references for a paper by arXiv id. direction: 'backward' (papers this paper cites), 'forward' (papers citing this paper), or 'both'."
    )]
    async fn get_references(
        &self,
        parameters: Parameters<tools::GetReferencesParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_references(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Search papers by title substring (case-insensitive, read-only).
    #[rmcp::tool(
        description = "Search papers by title substring (case-insensitive). Returns arxiv_id and title for matching Paper nodes. D127-safe read-only."
    )]
    async fn search_papers(
        &self,
        parameters: Parameters<tools::SearchPapersParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::search_papers(
            self.graph_store.as_ref(),
            Some(self.embedder.as_ref()),
            params,
        )
        .await
        .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get entities linked to a paper (read-only, MENTIONS edges).
    #[rmcp::tool(
        description = "Get entities linked to a paper via MENTIONS edges. Optional entity_type substring filter. D127-safe read-only."
    )]
    async fn get_entities(
        &self,
        parameters: Parameters<tools::GetEntitiesParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_entities(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Validate a node label against ontology schemas.
    #[rmcp::tool(
        description = "Validate a node label against registered schemas (YAML ontology + Rust). Returns is_known and violations."
    )]
    async fn validate_node(
        &self,
        parameters: Parameters<tools::ValidateNodeParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::validate_node(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get a summary digest for a paper (Layer B, D194 Wave 1).
    /// Returns multi-granularity text summary via Paper→hasSummary→Summary.
    #[rmcp::tool(
        description = "Get a summary digest for a paper by arXiv id. Returns text, granularity, and char_count. D127-safe read-only."
    )]
    async fn get_summary(
        &self,
        parameters: Parameters<tools::GetSummaryParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_summary(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Graph algorithm ranking (D194 Wave 2 S02).
    /// Runs PageRank or WCC on a typed subgraph.
    #[rmcp::tool(
        description = "Run graph algorithms (pagerank, wcc) on typed subgraphs. Returns ranked nodes or component sizes. D127-safe read-only."
    )]
    async fn graph_rank(
        &self,
        parameters: Parameters<tools::GraphRankParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::graph_rank(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get method lineage via predecessor_of edges (D194 Wave 2 S03).
    /// Returns ancestor/descendant methods in the Method influence graph.
    #[rmcp::tool(
        description = "Trace method lineage: ancestors (methods this builds on), descendants (methods that build on this), or both. Uses predecessor_of edges. D127-safe read-only."
    )]
    async fn get_method_lineage(
        &self,
        parameters: Parameters<tools::GetMethodLineageParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_method_lineage(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Get recommended reading path (D194 Wave 3 Layer D).
    /// Returns prerequisites + target + follow-ups in reading order.
    #[rmcp::tool(
        description = "Get recommended reading path for a paper: prerequisites (backward CITES), target paper, follow-ups (forward CITES). D127-safe read-only."
    )]
    async fn get_reading_path(
        &self,
        parameters: Parameters<tools::GetReadingPathParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::get_reading_path(self.graph_store.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }

    /// Vector search over node embeddings (read-only, D127-safe).
    /// Embeds the query via the configured embedder and searches the vector index.
    #[rmcp::tool(
        description = "Vector search over node embeddings: embed query then kNN search. D127-safe read-only; uses MockEmbedder when FD_API not configured."
    )]
    async fn vector_search(
        &self,
        parameters: Parameters<tools::VectorSearchParams>,
    ) -> Result<String, String> {
        let params = parameters.0;
        let out = tools::vector_search(self.graph_store.as_ref(), self.embedder.as_ref(), params)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&out).map_err(|e| format!("serialize: {e}"))
    }
}

// server_handler flag emits `impl ServerHandler for DailyArchiveMcpServer` with
// an auto-generated `get_info()` (tools capability). Import is kept for clarity.

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ServerHandler;

    fn test_server() -> DailyArchiveMcpServer {
        let mut s = DailyArchiveMcpServer::new(Arc::new(kg_storage::MockGraphStore::new()));
        s.embedder = Arc::new(kg_embeddings::MockEmbedder::new(8));
        s
    }

    #[test]
    fn test_prod_server_uses_fdapi_dimensions() {
        // Wave 0 Product Honesty (D194): prod server must use FdApi (1024d), not Mock (8d).
        let server = DailyArchiveMcpServer::new(Arc::new(kg_storage::MockGraphStore::new()));
        assert_eq!(
            server.embedder.dimensions(),
            1024,
            "prod server must use FdApiEmbedder (1024d), got {}d — product vector_search is lying",
            server.embedder.dimensions()
        );
    }

    #[tokio::test]
    async fn test_server_exposes_ontology_resources() {
        // Wave 1 MCP-4 (D194): server must expose ontology YAML as browsable resources.
        // Test the internal resource list directly (avoids RequestContext complexity).
        let server = test_server();
        let resources = server.list_mcp_resources();
        let uris: Vec<String> = resources.iter().map(|r| r.uri.clone()).collect();
        assert!(
            uris.iter()
                .any(|u| u.contains("schema") || u.contains("ontology")),
            "list_mcp_resources must include ontology/schema, got: {uris:?}"
        );
    }

    #[test]
    fn test_ontology_resources_are_embedded_and_cwd_independent() {
        let server = test_server();
        for uri in [
            "graph://ontology/node_schemas",
            "graph://ontology/edge_contracts",
            "graph://ontology/decision_categories",
            "graph://ontology/node_vocabulary",
        ] {
            assert!(server.read_mcp_resource(uri).is_some(), "missing {uri}");
        }
    }

    #[test]
    fn test_schema_resource_uses_all_authoritative_ontology_labels() {
        let labels = data_ontology_labels();
        assert_eq!(labels.len(), 36);
        assert!(labels.contains(&"Paper".to_string()));
        assert!(labels.contains(&"Conflict".to_string()));
        assert!(!labels.contains(&"HealingAction".to_string()));
        assert!(!labels.contains(&"ExtractionOverride".to_string()));
    }

    #[test]
    fn test_server_exposes_mcp_prompts() {
        // Wave 1 MCP-4 (D194): server must expose prompt templates for agent guidance.
        let server = test_server();
        let prompts = server.list_mcp_prompts();
        assert!(
            prompts.len() >= 2,
            "must expose at least 2 prompts, got {}",
            prompts.len()
        );
        let names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"literature_review"),
            "must have literature_review, got: {names:?}"
        );
    }

    #[test]
    fn test_server_created_with_snapshot_config() {
        let server = DailyArchiveMcpServer::with_snapshot(
            Arc::new(kg_storage::MockGraphStore::new()),
            "test.sgsnap",
        );
        assert_eq!(server.config.snapshot_path.as_deref(), Some("test.sgsnap"));
    }

    #[test]
    fn test_server_default_config_no_snapshot() {
        let server = test_server();
        assert!(server.config.snapshot_path.is_none());
    }

    #[test]
    fn test_server_tools_capability_enabled() {
        let server = test_server();
        let info = server.get_info();
        assert!(
            info.capabilities.tools.is_some(),
            "server must advertise tools capability"
        );
    }
}
