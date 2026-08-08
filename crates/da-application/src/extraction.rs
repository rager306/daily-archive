//! Extraction use case — parse → extract entities → write to graph.
//!
//! ADR-038 Module B: extract textually mentioned entities from paper sections.
//! Phase 3 Slice 2: wires Extractor port to IngestUseCase graph writes.

use da_domain::entity::EntityType;
use da_domain::vid;
use da_ports::extractor::{ExtractedEntity, Extractor};
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::ParsedArticle;

/// Extraction use case: given a parsed article, extract entities + write to graph.
pub struct ExtractionUseCase {
    pub extractor: Box<dyn Extractor>,
    pub graph_store: std::sync::Arc<dyn DirectGraphStore>,
    /// Optional embedder for entity label embeddings (Phase 3 GNN readiness).
    /// When Some, each unique entity label gets a bge-m3 embedding written
    /// to the Entity node's vector index.
    pub embedder: Option<Box<dyn da_ports::embedder::Embedder>>,
    /// Injected wall clock (ADR-051 / M330). Default: SystemClock.
    pub clock: std::sync::Arc<dyn da_ports::Clock>,
}

/// Result of extracting entities from one paper.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub paper_id: String,
    pub entities_extracted: usize,
    pub entity_types: Vec<String>,
    pub graph_node_ids: Vec<u64>,
    pub mentions_edges: usize,
    pub found_in_edges: usize,
    pub evidence_bundles_created: usize,
    pub participates_in_edges: usize,
    pub claims_created: usize,
    pub problems_created: usize,
    pub observations_created: usize,
}

impl ExtractionUseCase {
    pub fn new(extractor: Box<dyn Extractor>, graph_store: Box<dyn DirectGraphStore>) -> Self {
        Self {
            extractor,
            graph_store: std::sync::Arc::from(graph_store),
            embedder: None,
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn from_shared(
        extractor: Box<dyn Extractor>,
        graph_store: std::sync::Arc<dyn DirectGraphStore>,
    ) -> Self {
        Self {
            extractor,
            graph_store,
            embedder: None,
            clock: std::sync::Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: std::sync::Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Attach an embedder for entity label embeddings (Phase 3).
    pub fn with_embedder(mut self, embedder: Box<dyn da_ports::embedder::Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Build a kg-pipeline DSL describing the extraction workflow steps.
    /// This makes the extraction pipeline explicit: each step has a name,
    /// stage, and dependencies. The pipeline can be executed by ExecutionEngine
    /// or inspected for observability.
    ///
    /// Steps:
    /// 1. extract — run extractor on sections
    /// 2. write_entities — write Entity nodes to graph
    /// 3. write_edges — write MENTIONS/FOUND_IN edges
    /// 4. write_evidence — write EvidenceBundle nodes
    /// 5. embed (optional) — generate entity embeddings
    pub fn build_extraction_pipeline() -> Result<kg_pipeline::builder::Pipeline, String> {
        use kg_pipeline::builder::PipelineBuilder;
        use kg_pipeline::failure::FailurePolicy;

        PipelineBuilder::new("extraction")
            .step("extract", "extract", &[])
            .step_with_retry("write_entities", "write", &["extract"], "3x100ms")
            .step_with_retry("write_edges", "write", &["write_entities"], "3x100ms")
            .step("write_evidence", "write", &["write_entities"])
            .step("embed", "embed", &["write_entities"])
            .failure_policy(FailurePolicy::Abort)
            .build()
    }

    /// Extract entities from a parsed article and write Entity nodes to graph.
    /// ADR-038: entities are textually mentioned, grounded to source spans.
    /// Links each Entity to the Paper via MENTIONS edge (bibliographic).
    #[tracing::instrument(
        skip(self, parsed),
        fields(paper_id = %parsed.paper_id),
        name = "extract"
    )]
    pub async fn extract_from_parsed(
        &self,
        parsed: &ParsedArticle,
    ) -> anyhow::Result<ExtractionResult> {
        // 1. Build (title, text) pairs from parsed sections
        let sections: Vec<(String, String)> = parsed
            .sections
            .iter()
            .map(|s| (s.title.clone(), s.text.clone()))
            .collect();

        // 2. Extract via Extractor port
        let extracted = self.extractor.extract(&sections).await?;
        tracing::info!(
            paper_id = %parsed.paper_id,
            extractor = self.extractor.name(),
            count = extracted.len(),
            "Entities extracted"
        );
        self.materialize_extracted(parsed, extracted).await
    }

    /// Materialize already-computed extraction artifacts without invoking the
    /// extractor again. This is the always-run side-effect boundary used after
    /// a verified cache hit; callers must not place this method inside a
    /// cacheable `PipelineStageExecutor`.
    pub async fn materialize_extracted(
        &self,
        parsed: &ParsedArticle,
        extracted: Vec<ExtractedEntity>,
    ) -> anyhow::Result<ExtractionResult> {
        // 3. Find the Paper node by arxiv_id (to link entities via MENTIONS)
        let paper_node_id = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", &parsed.paper_id)
            .await;

        // 4. Write Entity nodes to graph (HOT path) + MENTIONS edges
        let now = self.clock.now().timestamp();
        let mut node_ids = Vec::new();
        let mut entity_types = Vec::new();
        let mut mentions_edges = 0usize;
        let mut found_in_edges = 0usize;
        for ext in &extracted {
            let entity_vid = vid::entity_vid(ext.entity_type.as_str(), &ext.label);
            // Idempotent: check if entity already exists
            let node_id = match self
                .graph_store
                .find_node_by_string_property("Entity", "vid", &entity_vid)
                .await
            {
                Some(existing) => existing,
                None => {
                    let new_node = self.graph_store.create_node("Entity").await?;
                    self.graph_store
                        .set_node_property_string(new_node, "vid", entity_vid.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(new_node, "label", ext.label.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_string(
                            new_node,
                            "entity_type",
                            ext.entity_type.as_str().to_string(),
                        )
                        .await?;
                    if !ext.section_title.is_empty() {
                        self.graph_store
                            .set_node_property_string(
                                new_node,
                                "section",
                                ext.section_title.clone(),
                            )
                            .await?;
                    }
                    // Source span (for evidence grounding — Phase 3 Slice 3+)
                    self.graph_store
                        .set_node_property_int(new_node, "char_start", ext.char_start as i64)
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "char_end", ext.char_end as i64)
                        .await?;
                    self.graph_store
                        .set_node_property_string(new_node, "surface", ext.surface.clone())
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "valid_from", now)
                        .await?;
                    self.graph_store
                        .set_node_property_int(new_node, "schema_version", 1)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "evidence_ready", false)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "import_eligible", false)
                        .await?;
                    self.graph_store
                        .set_node_property_bool(new_node, "retrieval_eligible", true)
                        .await?;
                    new_node
                }
            };
            // Link Entity to Paper via MENTIONS edge (if Paper node exists)
            if let Some(paper_id) = paper_node_id {
                let edge_type = da_domain::relation::bibliographic::MENTIONS;
                if self
                    .graph_store
                    .get_edges_between(paper_id, node_id, edge_type)
                    .await
                    .is_empty()
                {
                    // MENTIONS is temporal fact (meta-ontology temporal_class: fact) — M328.
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    let edge_id = crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        paper_id,
                        node_id,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                    // Set edge weight = extraction confidence (Phase 3 GNN readiness)
                    // Rule-based extractor has uniform confidence 1.0
                    self.graph_store
                        .set_edge_property_float(edge_id, "weight", 1.0)
                        .await?;
                    mentions_edges += 1;
                }
            }

            // Link Entity to Section via FOUND_IN edge (evidence grounding).
            // Enables: retrieval by section, PPR adjacency through Section nodes,
            // and evidence chain construction from Entity → Section → Work.
            if !ext.section_title.is_empty() {
                // Find Section node by title (hexagonal: no Cypher in application).
                if let Some(section_node) = self
                    .graph_store
                    .find_node_by_string_property("Section", "title", &ext.section_title)
                    .await
                {
                    let edge_type = da_domain::relation::structure::FOUND_IN;
                    if self
                        .graph_store
                        .get_edges_between(node_id, section_node, edge_type)
                        .await
                        .is_empty()
                    {
                        // Structural foundIn — created_at only (M331).
                        let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                        crate::temporal_stamp::create_edge_with_temporal(
                            self.graph_store.as_ref(),
                            node_id,
                            section_node,
                            edge_type,
                            &temporal,
                        )
                        .await?;
                        found_in_edges += 1;
                    }
                }
            }
            node_ids.push(node_id);
            entity_types.push(ext.entity_type.as_str().to_string());
        }

        // Phase 3: Embed entity labels if embedder is available.
        // Each unique entity label gets a bge-m3 embedding stored as a vector
        // on the Entity node. This enables GNN similarity search and PPR.
        let mut embeddings_written = 0usize;
        if let Some(ref embedder) = self.embedder {
            let mut seen_labels: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for (i, ext) in extracted.iter().enumerate() {
                let label_key = ext.label.to_lowercase();
                if seen_labels.contains(&label_key) {
                    continue; // skip duplicate labels
                }
                seen_labels.insert(label_key);

                let node_id = node_ids.get(i).copied();
                if let Some(nid) = node_id {
                    match embedder.embed(&ext.label).await {
                        Ok(vec) => {
                            if let Err(e) = self
                                .graph_store
                                .add_vector("Entity", "embedding", nid, vec)
                                .await
                            {
                                tracing::warn!(node_id = nid, error = %e, "Entity embedding write failed");
                            } else {
                                embeddings_written += 1;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(label = %ext.label, error = %e, "Entity embed failed");
                        }
                    }
                }
            }
        }

        // Phase 4: Create EvidenceBundle nodes for co-occurring entities.
        // When 2+ entities appear in the same section, they form an n-ary
        // evidence unit linked via PARTICIPATES_IN edges (ADR-042 revised).
        let mut evidence_bundles_created = 0usize;
        let mut claims_created = 0usize;
        let mut participates_in_edges = 0usize;
        if let Some(_paper_node) = paper_node_id {
            use std::collections::HashMap as Map;
            let mut by_section: Map<String, Vec<(usize, &str)>> = Map::new();
            for (i, ext) in extracted.iter().enumerate() {
                if !ext.section_title.is_empty() {
                    by_section
                        .entry(ext.section_title.clone())
                        .or_default()
                        .push((i, ext.label.as_str()));
                }
            }
            for (section_title, members) in &by_section {
                if members.len() < 2 {
                    continue;
                }
                let bundle_text = members
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>()
                    .join(", ");
                let member_vids = members
                    .iter()
                    .map(|(index, _)| {
                        let entity = &extracted[*index];
                        da_domain::vid::entity_vid(entity.entity_type.as_str(), &entity.label)
                    })
                    .collect::<Vec<_>>();
                let bundle_vid = da_domain::vid::evidence_bundle_vid(
                    &parsed.paper_id,
                    section_title,
                    &member_vids,
                );
                let bundle_node = match self
                    .graph_store
                    .find_node_by_string_property("EvidenceBundle", "vid", &bundle_vid)
                    .await
                {
                    Some(node) => node,
                    None => {
                        let node = self.graph_store.create_node("EvidenceBundle").await?;
                        self.graph_store
                            .set_node_property_string(node, "vid", bundle_vid.clone())
                            .await?;
                        self.graph_store
                            .set_node_property_string(
                                node,
                                "bundle_type",
                                "experiment_setup".to_string(),
                            )
                            .await?;
                        self.graph_store
                            .set_node_property_string(
                                node,
                                "normalized_text",
                                format!("Entities in {section_title}: {bundle_text}"),
                            )
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "document_id", parsed.paper_id.clone())
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
                        evidence_bundles_created += 1;
                        node
                    }
                };
                // Create PARTICIPATES_IN edges: Entity → EvidenceBundle.
                for (idx, _) in members {
                    if let Some(&entity_node) = node_ids.get(*idx) {
                        let edge_type = da_domain::relation::hypergraph::PARTICIPATES_IN;
                        if self
                            .graph_store
                            .get_edges_between(entity_node, bundle_node, edge_type)
                            .await
                            .is_empty()
                        {
                            let temporal =
                                crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                            crate::temporal_stamp::create_edge_with_temporal(
                                self.graph_store.as_ref(),
                                entity_node,
                                bundle_node,
                                edge_type,
                                &temporal,
                            )
                            .await?;
                            participates_in_edges += 1;
                        }
                    }
                }

                // Create Claim from EvidenceBundle (structural claim).
                let claim_text = format!(
                    "Paper {} uses {} in {}",
                    parsed.paper_id, bundle_text, section_title
                );
                let claim_vid = da_domain::vid::evidence_claim_vid(&bundle_vid);
                let claim_node = match self
                    .graph_store
                    .find_node_by_string_property("Claim", "vid", &claim_vid)
                    .await
                {
                    Some(node) => node,
                    None => {
                        let node = self.graph_store.create_node("Claim").await?;
                        self.graph_store
                            .set_node_property_string(node, "vid", claim_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "text", claim_text)
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "claim_type", "structural".to_string())
                            .await?;
                        self.graph_store
                            .set_node_property_string(
                                node,
                                "scope",
                                format!("section:{}", section_title),
                            )
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
                        claims_created += 1;
                        node
                    }
                };
                let edge_type = da_domain::relation::hypergraph::SUPPORTS;
                if self
                    .graph_store
                    .get_edges_between(bundle_node, claim_node, edge_type)
                    .await
                    .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        bundle_node,
                        claim_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                }
            }
        }

        // Phase 5: Create ResearchProblem from paper abstract.
        // Rule-based: detect problem-statement patterns in abstract.
        // "we propose" → improvement; "we investigate/study" → explanation.
        let mut problems_created = 0usize;
        if let Some(paper_id) = paper_node_id {
            let abstract_lower = parsed.abstract_text.to_lowercase();
            let problem_type = if abstract_lower.contains("we propose")
                || abstract_lower.contains("we improve")
                || abstract_lower.contains("we introduce")
            {
                Some("improvement")
            } else if abstract_lower.contains("we investigate")
                || abstract_lower.contains("we study")
                || abstract_lower.contains("we address")
            {
                Some("explanation")
            } else {
                None
            };
            if let Some(problem_type) = problem_type {
                let prob_text = if parsed.title.len() > 120 {
                    parsed.title.chars().take(120).collect::<String>()
                } else {
                    parsed.title.clone()
                };
                let problem_vid = format!("vid:problem:{}:{}", parsed.paper_id, problem_type);
                let prob_node = match self
                    .graph_store
                    .find_node_by_string_property("ResearchProblem", "vid", &problem_vid)
                    .await
                {
                    Some(node) => node,
                    None => {
                        let node = self.graph_store.create_node("ResearchProblem").await?;
                        self.graph_store
                            .set_node_property_string(node, "vid", problem_vid)
                            .await?;
                        self.graph_store
                            .set_node_property_string(node, "text", prob_text)
                            .await?;
                        self.graph_store
                            .set_node_property_string(
                                node,
                                "problem_type",
                                problem_type.to_string(),
                            )
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
                        problems_created += 1;
                        node
                    }
                };
                let edge_type = da_domain::relation::bibliographic::MENTIONS;
                if self
                    .graph_store
                    .get_edges_between(paper_id, prob_node, edge_type)
                    .await
                    .is_empty()
                {
                    let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                    crate::temporal_stamp::create_edge_with_temporal(
                        self.graph_store.as_ref(),
                        paper_id,
                        prob_node,
                        edge_type,
                        &temporal,
                    )
                    .await?;
                }
            }
        }

        // Phase 6: Create MetricObservation nodes for metric + number co-occurrences.
        // Rule-based: find metric names (accuracy, F1, BLEU, etc.) in Results/Experiments
        // sections, extract nearby number as the observed value.
        let mut observations_created = 0usize;
        if let Some(paper_node) = paper_node_id {
            let metrics_lower: &[&str] = &[
                "accuracy",
                "precision",
                "recall",
                "f1",
                "bleu",
                "rouge",
                "auc",
                "mse",
                "rmse",
                "mae",
            ];
            for ext in extracted.iter() {
                if ext.entity_type != EntityType::Metric {
                    continue;
                }
                let metric_lower = ext.label.to_lowercase();
                if !metrics_lower.contains(&metric_lower.as_str()) {
                    continue;
                }
                // Find numeric value near this metric mention
                let section_text = ext.surface.clone();
                if let Some(value) = extract_metric_value(&section_text, &metric_lower) {
                    let obs_vid = format!("vid:obs:{}:{}", parsed.paper_id, metric_lower);
                    let obs_node = match self
                        .graph_store
                        .find_node_by_string_property("MetricObservation", "vid", &obs_vid)
                        .await
                    {
                        Some(node) => node,
                        None => {
                            let node = self.graph_store.create_node("MetricObservation").await?;
                            self.graph_store
                                .set_node_property_string(node, "vid", obs_vid)
                                .await?;
                            self.graph_store
                                .set_node_property_string(
                                    node,
                                    "metric_definition_id",
                                    metric_lower.clone(),
                                )
                                .await?;
                            self.graph_store
                                .set_node_property_float(node, "value", value)
                                .await?;
                            let pseudo_run_id = format!("run:paper:{}", parsed.paper_id);
                            self.graph_store
                                .set_node_property_string(node, "run_id", pseudo_run_id)
                                .await?;
                            self.graph_store
                                .set_node_property_string(
                                    node,
                                    "document_id",
                                    parsed.paper_id.clone(),
                                )
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
                            observations_created += 1;
                            node
                        }
                    };
                    let edge_type = da_domain::relation::bibliographic::MENTIONS;
                    if self
                        .graph_store
                        .get_edges_between(paper_node, obs_node, edge_type)
                        .await
                        .is_empty()
                    {
                        let temporal = crate::temporal_stamp::open_fact_edge_now(self.clock.now());
                        crate::temporal_stamp::create_edge_with_temporal(
                            self.graph_store.as_ref(),
                            paper_node,
                            obs_node,
                            edge_type,
                            &temporal,
                        )
                        .await?;
                    }
                }
            }
        }

        tracing::info!(
            paper_id = %parsed.paper_id,
            entities = extracted.len(),
            mentions_edges,
            found_in_edges,
            evidence_bundles_created,
            claims_created,
            problems_created,
            participates_in_edges,
            embeddings_written,
            paper_linked = paper_node_id.is_some(),
            "Extraction complete"
        );

        Ok(ExtractionResult {
            paper_id: parsed.paper_id.clone(),
            entities_extracted: extracted.len(),
            entity_types,
            graph_node_ids: node_ids,
            mentions_edges,
            found_in_edges,
            evidence_bundles_created,
            claims_created,
            problems_created,
            participates_in_edges,
            observations_created,
        })
    }
}

/// Extract a numeric metric value from text near a metric name.
/// Looks for number immediately before or after the metric name.
/// Examples: "accuracy of 0.95" → 0.95, "F1=0.87" → 0.87.
fn extract_metric_value(text: &str, metric: &str) -> Option<f64> {
    let lower = text.to_lowercase();
    // Pattern: metric followed by number (accuracy 0.95, F1=0.87)
    let patterns: Vec<String> = vec![
        format!("{} =", metric),
        format!("{}:", metric),
        metric.to_string(),
    ];
    for pat in &patterns {
        if let Some(pos) = lower.find(pat) {
            let after = &lower[pos + pat.len()..];
            let cleaned = after.trim_start_matches(|c: char| {
                c.is_whitespace() || c == '=' || c == ':' || c == 'o' || c == 'f'
            });
            // If cleaned ends in a full number (no trailing non-digit),
            // num_end will be None — use the whole cleaned string.
            let num_end = cleaned
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(cleaned.len());
            let num_str = &cleaned[..num_end];
            if let Ok(value) = num_str.parse::<f64>() {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metric_value_after() {
        let text = "We achieved accuracy of 0.95 on the test set.";
        assert_eq!(extract_metric_value(text, "accuracy"), Some(0.95));
    }

    #[test]
    fn test_extract_metric_value_equals() {
        let text = "F1=0.87 which is better than baseline.";
        assert_eq!(extract_metric_value(text, "f1"), Some(0.87));
    }

    #[test]
    fn test_extract_metric_value_no_number() {
        let text = "We report accuracy in our experiments.";
        assert_eq!(extract_metric_value(text, "accuracy"), None);
    }

    #[test]
    fn test_build_extraction_pipeline() {
        let pipeline = ExtractionUseCase::build_extraction_pipeline()
            .expect("pipeline must build successfully");
        assert_eq!(pipeline.steps.len(), 5, "must have 5 steps");
        assert_eq!(pipeline.steps[0].name, "extract");
        assert_eq!(pipeline.steps[1].name, "write_entities");
        assert_eq!(pipeline.steps[2].name, "write_edges");
        assert_eq!(pipeline.steps[3].name, "write_evidence");
        assert_eq!(pipeline.steps[4].name, "embed");
        // write_edges depends on write_entities
        assert!(
            pipeline.steps[2]
                .depends_on
                .contains(&"write_entities".to_string()),
            "write_edges must depend on write_entities"
        );
    }
}
