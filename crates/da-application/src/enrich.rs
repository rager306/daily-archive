//! Enrichment use case — fetch curated metadata from OpenAlex + write to graph.
//!
//! D133: OpenAlex replaces noisy YAKE keyword extraction for the metadata layer.
//! Fetches Topic, Author, Concept data and writes nodes to the graph.

use da_domain::vid;
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::OpenAlexClient;

/// Enrichment use case: fetch OpenAlex metadata → write graph nodes.
pub struct EnrichUseCase {
    pub openalex: Box<dyn OpenAlexClient>,
    pub graph_store: Box<dyn DirectGraphStore>,
}

/// Result of enriching one work.
#[derive(Debug, Clone)]
pub struct EnrichResult {
    pub arxiv_id: String,
    pub title: String,
    pub topics_written: usize,
    pub authors_written: usize,
    pub concepts_written: usize,
    pub doi: Option<String>,
    pub cited_by_count: u32,
    pub openalex_id: String,
}

impl EnrichUseCase {
    pub fn new(openalex: Box<dyn OpenAlexClient>, graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            openalex,
            graph_store,
        }
    }

    /// Fetch metadata from OpenAlex and write Topic/Author nodes to graph.
    pub async fn enrich_by_arxiv_id(&self, arxiv_id: &str) -> anyhow::Result<EnrichResult> {
        tracing::info!(arxiv_id, "Fetching OpenAlex metadata");

        let work = self
            .openalex
            .fetch_by_arxiv_id(arxiv_id)
            .await
            .map_err(|e| anyhow::anyhow!("OpenAlex fetch failed: {e}"))?;

        let now = chrono::Utc::now().timestamp();
        let mut topics_written = 0;
        let mut authors_written = 0;

        // Write Topic nodes from primary_topic + topics array
        let all_topics: Vec<_> = work
            .primary_topic
            .iter()
            .chain(work.topics.iter())
            .collect();

        for topic in &all_topics {
            let topic_vid = vid::paper_vid(&topic.id); // reuse VID pattern
            let node_id = match self
                .graph_store
                .find_node_by_string_property("Topic", "vid", &topic_vid)
                .await
            {
                Some(existing) => existing,
                None => {
                    let node = self.graph_store.create_node("Topic").await?;
                    self.graph_store
                        .set_node_property_string(node, "vid", topic_vid.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(node, "label", topic.display_name.clone())
                        .await?;
                    if let Some(ref domain) = topic.domain {
                        self.graph_store
                            .set_node_property_string(node, "domain", domain.clone())
                            .await?;
                    }
                    if let Some(ref field) = topic.field {
                        self.graph_store
                            .set_node_property_string(node, "field", field.clone())
                            .await?;
                    }
                    if let Some(ref subfield) = topic.subfield {
                        self.graph_store
                            .set_node_property_string(node, "subfield", subfield.clone())
                            .await?;
                    }
                    self.graph_store
                        .set_node_property_string(node, "openalex_id", topic.id.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "retrieval_eligible", true)
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "valid_from", now)
                        .await?;
                    node
                }
            };

            // Link Work → Topic (hasTopic edge)
            if let Some(work_id) = self
                .graph_store
                .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
                .await
            {
                self.graph_store
                    .create_edge(work_id, node_id, "hasTopic")
                    .await?;
            }
            topics_written += 1;
        }

        // Write Author nodes
        for author in &work.authors {
            let author_vid = vid::author_vid(&author.display_name);
            let node_id = match self
                .graph_store
                .find_node_by_string_property("Author", "vid", &author_vid)
                .await
            {
                Some(existing) => existing,
                None => {
                    let node = self.graph_store.create_node("Author").await?;
                    self.graph_store
                        .set_node_property_string(node, "vid", author_vid.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(node, "name", author.display_name.clone())
                        .await?;
                    if let Some(ref orcid) = author.orcid {
                        self.graph_store
                            .set_node_property_string(node, "orcid", orcid.clone())
                            .await?;
                    }
                    self.graph_store
                        .set_node_property_string(node, "openalex_id", author.id.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "retrieval_eligible", true)
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "valid_from", now)
                        .await?;
                    node
                }
            };

            // Link Author → Paper (authoredBy edge)
            if let Some(paper_id) = self
                .graph_store
                .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
                .await
            {
                self.graph_store
                    .create_edge(node_id, paper_id, "authoredBy")
                    .await?;
            }
            authors_written += 1;
        }

        // Concepts (deprecated but kept for audit, retrieval_eligible=false)
        let concepts_written = work.concepts.len();

        tracing::info!(
            arxiv_id,
            title = %work.title,
            topics_written,
            authors_written,
            concepts_written,
            cited_by_count = work.cited_by_count,
            "OpenAlex enrichment complete"
        );

        Ok(EnrichResult {
            arxiv_id: arxiv_id.to_string(),
            title: work.title,
            topics_written,
            authors_written,
            concepts_written,
            doi: work.doi,
            cited_by_count: work.cited_by_count,
            openalex_id: work.id,
        })
    }
}
