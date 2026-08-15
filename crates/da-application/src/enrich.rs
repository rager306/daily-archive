//! Enrichment use case — fetch curated metadata from OpenAlex + write to graph.
//!
//! D133: OpenAlex replaces noisy YAKE keyword extraction for the metadata layer.
//! Fetches Topic, Author, Concept data and writes nodes to the graph.

use da_domain::vid;
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::OpenAlexClient;

const MAX_REFERENCED_WORKS: usize = 500;

/// Enrichment use case: fetch OpenAlex metadata → write graph nodes.
pub struct EnrichUseCase {
    pub openalex: std::sync::Arc<dyn OpenAlexClient>,
    pub graph_store: std::sync::Arc<dyn DirectGraphStore>,
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
}

/// Result of enriching one work.
#[derive(Debug, Clone)]
pub struct EnrichResult {
    pub arxiv_id: String,
    pub title: String,
    pub topics_written: usize,
    pub topics_reused: usize,
    pub topic_edges_written: usize,
    pub authors_written: usize,
    pub authors_reused: usize,
    pub author_edges_written: usize,
    pub concepts_written: usize,
    pub concepts_candidates: usize,
    pub institutions_written: usize,
    pub institutions_reused: usize,
    pub affiliation_edges_written: usize,
    /// Number of CITES edges created from OpenAlex `referenced_works`.
    /// Each valid work ID becomes a candidate-only Citation stub + CITES edge.
    pub cites_edges_written: usize,
    pub doi: Option<String>,
    pub cited_by_count: u32,
    pub openalex_id: String,
    /// True when OpenAlex had no data — stub created with GROBID-only metadata.
    pub openalex_pending: bool,
}

impl EnrichUseCase {
    pub fn new(openalex: Box<dyn OpenAlexClient>, graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self::from_shared(
            std::sync::Arc::from(openalex),
            std::sync::Arc::from(graph_store),
        )
    }

    pub fn from_shared(
        openalex: std::sync::Arc<dyn OpenAlexClient>,
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
    ) -> Self {
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
        // PIR-008 / M398-S04: node DateTime wire is a Clock-derived RFC3339
        // UTC string — integer timestamps are rejected by schema validation.
        let now_rfc3339 = self.clock.now().to_rfc3339();
        let paper = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Paper {arxiv_id} missing; refuse enrichment completion without identity write-back"
                )
            })?;
        self.graph_store
            .set_node_property_string(paper, "openalex_id", work.id.clone())
            .await?;
        if let Some(doi) = &work.doi {
            self.graph_store
                .set_node_property_string(paper, "doi", doi.clone())
                .await?;
        }
        self.graph_store
            .set_node_property_int(paper, "cited_by_count", work.cited_by_count as i64)
            .await?;
        // Commit marker is deliberately last. Scheduler completion additionally
        // verifies this marker and the OpenAlex identity.
        self.graph_store
            .set_node_property_bool(paper, "openalex_pending", false)
            .await?;
        let mut topics_written = 0;
        let mut topics_reused = 0;
        let mut topic_edges_written = 0;
        let mut authors_written = 0;
        let mut authors_reused = 0;
        let mut author_edges_written = 0;

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
                Some(existing) => {
                    topics_reused += 1;
                    existing
                }
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
                        .set_node_property_string(node, "valid_from", now_rfc3339.clone())
                        .await?;
                    topics_written += 1;
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
                if self
                    .graph_store
                    .get_edges_between(work_id, node_id, da_domain::relation::structure::HAS_TOPIC)
                    .await
                    .is_empty()
                {
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        work_id,
                        node_id,
                        da_domain::relation::structure::HAS_TOPIC,
                        &temporal,
                    )
                    .await?;
                    topic_edges_written += 1;
                }
            }
        }

        // Write Author nodes
        for author in &work.authors {
            let author_vid = vid::author_vid(&author.display_name);
            let node_id = match self
                .graph_store
                .find_node_by_string_property("Author", "vid", &author_vid)
                .await
            {
                Some(existing) => {
                    authors_reused += 1;
                    existing
                }
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
                        .set_node_property_string(node, "valid_from", now_rfc3339.clone())
                        .await?;
                    authors_written += 1;
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
                if self
                    .graph_store
                    .get_edges_between(
                        node_id,
                        paper_id,
                        da_domain::relation::structure::AUTHORED_BY,
                    )
                    .await
                    .is_empty()
                {
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        node_id,
                        paper_id,
                        da_domain::relation::structure::AUTHORED_BY,
                        &temporal,
                    )
                    .await?;
                    author_edges_written += 1;
                }
            }
        }

        // Institutions — materialized from per-authorship rows so the
        // Author → Institution edge (AFFILIATED_WITH) is preserved.
        // The earlier flat-`work.institutions` path dropped this association.
        let mut institutions_written = 0usize;
        let mut institutions_reused = 0usize;
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
                    Some(existing) => {
                        institutions_reused += 1;
                        existing
                    }
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
                    if self
                        .graph_store
                        .get_edges_between(
                            author_id,
                            inst_node,
                            da_domain::relation::bibliographic::AFFILIATED_WITH,
                        )
                        .await
                        .is_empty()
                    {
                        crate::temporal_stamp::create_edge_with_temporal(
                            self.graph_store.as_ref(),
                            author_id,
                            inst_node,
                            da_domain::relation::bibliographic::AFFILIATED_WITH,
                            &temporal,
                        )
                        .await?;
                        affiliation_edges_written += 1;
                    }
                }
            }
        }

        // Deprecated OpenAlex concepts remain candidate metadata only. They
        // are not materialized and cannot be counted as successful writes.
        let concepts_written = 0;
        let concepts_candidates = work.concepts.len();

        // ADR-053 Layer F prerequisite: preserve OpenAlex `referenced_works`
        // as Citation evidence. A provider reference is not an authoritative
        // Paper; it becomes a candidate-only Citation stub + CITES edge.
        let mut cites_edges_written = 0usize;
        if let Some(paper_node) = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await
        {
            anyhow::ensure!(
                work.referenced_works.len() <= MAX_REFERENCED_WORKS,
                "OpenAlex referenced_works exceeds limit: {} > {}",
                work.referenced_works.len(),
                MAX_REFERENCED_WORKS
            );
            da_domain::validate_compiled_edge_endpoint(
                da_domain::relation::bibliographic::CITES,
                "Paper",
                "Citation",
            )
            .map_err(anyhow::Error::msg)?;
            for ref_openalex_id in &work.referenced_works {
                let canonical_openalex_id =
                    vid::canonical_openalex_work_id(ref_openalex_id).map_err(anyhow::Error::msg)?;
                let citation_vid =
                    vid::citation_vid(&canonical_openalex_id).map_err(anyhow::Error::msg)?;
                let ref_node = match self
                    .graph_store
                    .find_node_by_string_property("Citation", "vid", &citation_vid)
                    .await
                {
                    Some(existing) => existing,
                    None => {
                        let node = self.graph_store.create_node("Citation").await?;
                        self.graph_store
                            .set_node_property_string(node, "vid", citation_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "openalex_id", canonical_openalex_id)
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
                            .set_node_property_string(node, "valid_from", now_rfc3339.clone())
                            .await?;
                        node
                    }
                };
                // CITES is a revisable fact: Paper → Citation evidence.
                let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                if self
                    .graph_store
                    .get_edges_between(
                        paper_node,
                        ref_node,
                        da_domain::relation::bibliographic::CITES,
                    )
                    .await
                    .is_empty()
                {
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        paper_node,
                        ref_node,
                        da_domain::relation::bibliographic::CITES,
                        &temporal,
                    )
                    .await?;
                    cites_edges_written += 1;
                }
            }
        }

        tracing::info!(
            arxiv_id,
            title = %work.title,
            topics_written,
            topics_reused,
            topic_edges_written,
            authors_written,
            authors_reused,
            author_edges_written,
            concepts_written,
            concepts_candidates,
            institutions_written,
            institutions_reused,
            affiliation_edges_written,
            cites_edges_written,
            cited_by_count = work.cited_by_count,
            "OpenAlex enrichment complete"
        );

        Ok(EnrichResult {
            arxiv_id: arxiv_id.to_string(),
            title: work.title,
            topics_written,
            topics_reused,
            topic_edges_written,
            authors_written,
            authors_reused,
            author_edges_written,
            concepts_written,
            concepts_candidates,
            institutions_written,
            institutions_reused,
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

        // Auto-register through the same idempotent scheduler contract used by
        // scheduler-run. Existing pending tasks are reused.
        let policy = da_domain::scheduler::RetryPolicy::default();
        crate::GraphScheduler::add_pending_to(self.graph_store.as_ref(), &policy, arxiv_id, now)
            .await?;

        Ok(EnrichResult {
            arxiv_id: arxiv_id.to_string(),
            title: "(pending OpenAlex)".to_string(),
            topics_written: 0,
            topics_reused: 0,
            topic_edges_written: 0,
            authors_written: 0,
            authors_reused: 0,
            author_edges_written: 0,
            concepts_written: 0,
            concepts_candidates: 0,
            institutions_written: 0,
            institutions_reused: 0,
            affiliation_edges_written: 0,
            cites_edges_written: 0,
            doi: None,
            cited_by_count: 0,
            openalex_id: String::new(),
            openalex_pending: true,
        })
    }
}
