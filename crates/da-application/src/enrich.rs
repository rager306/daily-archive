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
    pub institutions_written: usize,
    pub doi: Option<String>,
    pub cited_by_count: u32,
    pub openalex_id: String,
    /// True when OpenAlex had no data — stub created with GROBID-only metadata.
    pub openalex_pending: bool,
}

impl EnrichUseCase {
    pub fn new(openalex: Box<dyn OpenAlexClient>, graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            openalex,
            graph_store,
        }
    }

    /// Fetch metadata from OpenAlex and write Topic/Author nodes to graph.
    /// Lazy load: if OpenAlex has no data, creates stub with openalex_pending=true.
    pub async fn enrich_by_arxiv_id(&self, arxiv_id: &str) -> anyhow::Result<EnrichResult> {
        tracing::info!(arxiv_id, "Fetching OpenAlex metadata");

        let fetch_result = self.openalex.fetch_by_arxiv_id(arxiv_id).await;

        match fetch_result {
            Ok(work) => self.enrich_from_work(arxiv_id, work).await,
            Err(da_ports::openalex::OpenAlexError::NotFound(_)) => {
                tracing::warn!(
                    arxiv_id,
                    "OpenAlex has no data — creating stub with openalex_pending=true"
                );
                self.create_pending_stub(arxiv_id).await
            }
            Err(e) => Err(anyhow::anyhow!("OpenAlex fetch failed: {e}")),
        }
    }

    /// Enrich from a successfully fetched OpenAlex work.
    async fn enrich_from_work(
        &self,
        arxiv_id: &str,
        work: da_ports::openalex::OpenAlexWork,
    ) -> anyhow::Result<EnrichResult> {
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
                        .set_node_property_bool(node, "import_eligible", false) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "schema_version", 1)
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
                    .create_edge(work_id, node_id, da_domain::relation::structure::HAS_TOPIC)
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
                        .set_node_property_bool(node, "import_eligible", false) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "schema_version", 1)
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
                    .create_edge(
                        node_id,
                        paper_id,
                        da_domain::relation::structure::AUTHORED_BY,
                    )
                    .await?;
            }
            authors_written += 1;
        }

        // Institutions (from OpenAlex authorship data)
        let mut institutions_written = 0usize;
        for institution in &work.institutions {
            // Idempotent: check if Institution already exists
            // NOTE: We create the Institution node but do not yet link it to Author
            // (no AFFILIATED_WITH edge type defined yet — ADR-043 Wave 2).
            let _inst_node = match self
                .graph_store
                .find_node_by_string_property("Institution", "openalex_id", &institution.id)
                .await
            {
                Some(existing) => existing,
                None => {
                    let node = self.graph_store.create_node("Institution").await?;
                    let inst_vid = vid::author_vid(&institution.display_name); // reuse VID pattern
                    self.graph_store
                        .set_node_property_string(node, "vid", inst_vid)
                        .await?;
                    self.graph_store
                        .set_node_property_string(node, "name", institution.display_name.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(node, "openalex_id", institution.id.clone())
                        .await?;
                    if let Some(ref country) = institution.country_code {
                        self.graph_store
                            .set_node_property_string(node, "country", country.clone())
                            .await?;
                    }
                    if let Some(ref ror) = institution.ror {
                        self.graph_store
                            .set_node_property_string(node, "ror", ror.clone())
                            .await?;
                    }
                    self.graph_store
                        .set_node_property_bool(node, "retrieval_eligible", true)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(node, "import_eligible", false) // D127
                        .await?;
                    self.graph_store
                        .set_node_property_int(node, "schema_version", 1)
                        .await?;
                    node
                }
            };
            institutions_written += 1;
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
            institutions_written,
            doi: work.doi,
            cited_by_count: work.cited_by_count,
            openalex_id: work.id,
            openalex_pending: false,
        })
    }

    /// Create a stub Work node when OpenAlex has no data.
    /// Marks the Paper node with openalex_pending=true for later enrichment.
    async fn create_pending_stub(&self, arxiv_id: &str) -> anyhow::Result<EnrichResult> {
        let now = chrono::Utc::now().timestamp();
        // Find the Paper node (created by ingest) and mark it pending
        if let Some(paper_id) = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await
        {
            self.graph_store
                .set_node_property_bool(paper_id, "openalex_pending", true)
                .await?;
            tracing::info!(arxiv_id, paper_id, "Paper marked openalex_pending=true");
        }

        // Auto-register SchedulerTask node in graph (ADR-040: Samyama sole store)
        let policy = da_domain::scheduler::RetryPolicy::default();
        let task = da_domain::scheduler::PendingTask::new_openalex_enrich(arxiv_id, &policy, now);
        let task_node = self.graph_store.create_node("SchedulerTask").await?;
        self.graph_store
            .set_node_property_string(task_node, "arxiv_id", arxiv_id.to_string())
            .await?;
        self.graph_store
            .set_node_property_string(task_node, "task_type", task.task_type.as_str().to_string())
            .await?;
        self.graph_store
            .set_node_property_string(task_node, "status", "pending".to_string())
            .await?;
        self.graph_store
            .set_node_property_int(task_node, "retry_count", task.retry_count as i64)
            .await?;
        self.graph_store
            .set_node_property_int(task_node, "next_retry", task.next_retry)
            .await?;
        self.graph_store
            .set_node_property_bool(task_node, "retrieval_eligible", false)
            .await?;
        self.graph_store
            .set_node_property_bool(task_node, "import_eligible", false) // D127
            .await?;
        self.graph_store
            .set_node_property_int(task_node, "schema_version", 1)
            .await?;
        tracing::info!(arxiv_id, task_node, "SchedulerTask created in graph");

        Ok(EnrichResult {
            arxiv_id: arxiv_id.to_string(),
            title: "(pending OpenAlex)".to_string(),
            topics_written: 0,
            authors_written: 0,
            concepts_written: 0,
            institutions_written: 0,
            doi: None,
            cited_by_count: 0,
            openalex_id: String::new(),
            openalex_pending: true,
        })
    }
}
