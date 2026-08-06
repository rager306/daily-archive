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
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
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
    pub affiliation_edges_written: usize,
    /// Number of CITES edges created from OpenAlex `referenced_works`.
    /// Each referenced work becomes a stub Paper node + CITES edge (ADR-053 Layer F).
    pub cites_edges_written: usize,
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
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Fetch metadata from OpenAlex and write Topic/Author nodes to graph.
    /// Lazy load: if OpenAlex has no data, creates stub with openalex_pending=true.
    #[tracing::instrument(skip(self), fields(arxiv_id = %arxiv_id), name = "enrich")]
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
        let now = self.clock.now().timestamp();
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
                // Structural hasTopic — created_at only (M331).
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    work_id,
                    node_id,
                    da_domain::relation::structure::HAS_TOPIC,
                    &temporal,
                )
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
                // Structural authoredBy — created_at only (M331).
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    node_id,
                    paper_id,
                    da_domain::relation::structure::AUTHORED_BY,
                    &temporal,
                )
                .await?;
            }
            authors_written += 1;
        }

        // Institutions — materialized from per-authorship rows so the
        // Author → Institution edge (AFFILIATED_WITH) is preserved.
        // The earlier flat-`work.institutions` path dropped this association.
        let mut institutions_written = 0usize;
        let mut affiliation_edges_written = 0usize;
        for authorship in &work.authorships {
            // Find the Author node for this authorship (created above).
            let author_vid = vid::author_vid(&authorship.author.display_name);
            let author_node = self
                .graph_store
                .find_node_by_string_property("Author", "vid", &author_vid)
                .await;

            for institution in &authorship.institutions {
                // Idempotent: check if Institution already exists by openalex_id.
                let inst_node = match self
                    .graph_store
                    .find_node_by_string_property("Institution", "openalex_id", &institution.id)
                    .await
                {
                    Some(existing) => existing,
                    None => {
                        let node = self.graph_store.create_node("Institution").await?;
                        // Institution VID derived from display_name + openalex_id
                        // (avoids collision with Author VID on the same name).
                        let inst_vid =
                            vid::institution_vid(&institution.display_name, &institution.id);
                        self.graph_store
                            .set_node_property_string(node, "vid", inst_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(
                                node,
                                "name",
                                institution.display_name.clone(),
                            )
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
                        institutions_written += 1;
                        node
                    }
                };
                // Wire Author → Institution (AFFILIATED_WITH) if we resolved the Author.
                if let Some(author_id) = author_node {
                    // AFFILIATED_WITH is temporal fact — M329.
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    let _ = crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        author_id,
                        inst_node,
                        da_domain::relation::bibliographic::AFFILIATED_WITH,
                        &temporal,
                    )
                    .await;
                    affiliation_edges_written += 1;
                }
            }
        }

        // Concepts (deprecated but kept for audit, retrieval_eligible=false)
        let concepts_written = work.concepts.len();

        // ADR-053 Layer F prerequisite: create CITES edges from OpenAlex
        // `referenced_works`. Previously this field was fetched but discarded
        // (data loss — citation graph was incomplete). Each referenced work
        // becomes a stub Paper node (openalex_id only) + CITES edge.
        // Stub nodes are retrieval_eligible=false, import_eligible=false (D127).
        let mut cites_edges_written = 0usize;
        if let Some(paper_node) = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await
        {
            for ref_openalex_id in &work.referenced_works {
                // Idempotent: check if a stub Paper for this openalex_id exists.
                let ref_node = match self
                    .graph_store
                    .find_node_by_string_property("Paper", "openalex_id", ref_openalex_id)
                    .await
                {
                    Some(existing) => existing,
                    None => {
                        // Create stub Paper node for the referenced work.
                        let node = self.graph_store.create_node("Paper").await?;
                        self.graph_store
                            .set_node_property_string(node, "openalex_id", ref_openalex_id.clone())
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "vid", vid::paper_vid(ref_openalex_id))
                            .await?;
                        self.graph_store
                            .set_node_property_bool(node, "retrieval_eligible", false)
                            .await?;
                        self.graph_store
                            .set_node_property_bool(node, "import_eligible", false)
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
                // CITES edge: Paper → referenced Paper (structural, created_at only).
                // Not a fact edge — citation relationship is stable metadata.
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                let _ = crate::temporal_stamp::create_edge_with_temporal(
                    self.graph_store.as_ref(),
                    paper_node,
                    ref_node,
                    da_domain::relation::bibliographic::CITES,
                    &temporal,
                )
                .await;
                cites_edges_written += 1;
            }
        }

        tracing::info!(
            arxiv_id,
            title = %work.title,
            topics_written,
            authors_written,
            concepts_written,
            cites_edges_written,
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
            affiliation_edges_written,
            cites_edges_written,
            doi: work.doi,
            cited_by_count: work.cited_by_count,
            openalex_id: work.id,
            openalex_pending: false,
        })
    }

    /// Create a stub Work node when OpenAlex has no data.
    /// Marks the Paper node with openalex_pending=true for later enrichment.
    async fn create_pending_stub(&self, arxiv_id: &str) -> anyhow::Result<EnrichResult> {
        let now = self.clock.now().timestamp();
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
            affiliation_edges_written: 0,
            cites_edges_written: 0,
            doi: None,
            cited_by_count: 0,
            openalex_id: String::new(),
            openalex_pending: true,
        })
    }
}
