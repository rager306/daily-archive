//! Scheduler task executors for content task types (M410 S01).
//!
//! Dispatch stays closed: each executor implements
//! [`da_application::SchedulerTaskExecutor`] for exactly one `TaskType` and
//! is registered through `SchedulerRunner::with_executor`. Unknown task
//! types are never claimed.
//!
//! [`ReparseSchedulerExecutor`] re-runs ingest on an already-materialized
//! Paper after a parser or profile change (ADR-056). Same catalog PDF
//! locator as Ingest; a missing Paper is typed `Retryable`, not a first-time
//! ingest in disguise.
//!
//! [`ExtractSchedulerExecutor`] makes the `Extract` type executable: it reads
//! the already-materialized revision plane from the graph (Paper →
//! `hasPart` → Section title/text) into a [`ParsedArticle`], runs the
//! injection-provided `ExtractionUseCase` and materializes entities. This is
//! honest and self-contained — no network, no catalog, no cached artifact:
//! it operates on graph truth only, exactly what a `PendingTask` carries
//! (`arxiv_id`) enables after a prepare already materialized the plane.
//! A missing paper or empty sections is a typed `Retryable` (the prepare may
//! not have landed yet), never a fabricated parse.

use std::sync::Arc;

use async_trait::async_trait;
use da_ports::CatalogPdfLocator;
use da_ports::embedder::Embedder;
use da_ports::extractor::Extractor;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ParsedArticle, ParserPort, Section};

use crate::extraction::ExtractionUseCase;
use crate::ingest::IngestUseCase;
use crate::{SchedulerExecuteOutcome, SchedulerTaskExecutor};
use da_domain::scheduler::{PendingTask, TaskType};

/// Reads the parsed article for an arxiv id from the materialized graph:
/// Paper → `hasPart` → Section(title, text) with a deterministic ordinal.
pub struct GraphParsedArticleProvider {
    graph_store: Arc<dyn DirectGraphStore>,
}

impl GraphParsedArticleProvider {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Rebuild a [`ParsedArticle`] from the materialized Section nodes.
    /// `None` when the paper or any required section text is absent — the
    /// caller turns that into a typed Retryable.
    pub async fn provide(&self, arxiv_id: &str) -> Option<ParsedArticle> {
        let paper = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await?;
        let title = self
            .graph_store
            .get_node_property_string(paper, "title")
            .await
            .unwrap_or_default();
        let mut sections = Vec::new();
        for (section_node, edge_type) in self.graph_store.get_outgoing_edges(paper).await {
            if edge_type != da_domain::relation::structure::HAS_PART {
                continue;
            }
            let section_title = self
                .graph_store
                .get_node_property_string(section_node, "title")
                .await
                .unwrap_or_default();
            let text = self
                .graph_store
                .get_node_property_string(section_node, "text")
                .await
                .unwrap_or_default();
            let order = self
                .graph_store
                .get_node_property_int(section_node, "order")
                .await
                .unwrap_or(0);
            sections.push(Section {
                title: section_title,
                text,
                level: 1,
                path_id: None,
                section_id: Some(order.to_string()),
                ..Default::default()
            });
        }
        if sections.is_empty() {
            return None;
        }
        sections.sort_by_key(|section| {
            section
                .section_id
                .as_deref()
                .and_then(|id| id.parse::<i64>().ok())
                .unwrap_or(0)
        });
        Some(ParsedArticle {
            paper_id: arxiv_id.to_string(),
            title,
            abstract_text: String::new(),
            body_text: String::new(),
            sections,
            citations: Vec::new(),
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            pdf_hash: String::new(),
        })
    }
}

/// Executor for the `Extract` task type: extract entities from the
/// materialized revision plane.
pub struct ExtractSchedulerExecutor {
    graph_store: Arc<dyn DirectGraphStore>,
    use_case: ExtractionUseCase,
}

impl ExtractSchedulerExecutor {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, extractor: Box<dyn Extractor>) -> Self {
        let use_case = ExtractionUseCase::from_shared(extractor, graph_store.clone());
        Self {
            graph_store,
            use_case,
        }
    }
}

#[async_trait]
impl SchedulerTaskExecutor for ExtractSchedulerExecutor {
    fn task_type(&self) -> TaskType {
        TaskType::Extract
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        let provider = GraphParsedArticleProvider::new(self.graph_store.clone());
        let Some(parsed) = provider.provide(&task.arxiv_id).await else {
            return SchedulerExecuteOutcome::Retryable {
                error_class: "parsed_plane_not_materialized".into(),
                message: format!(
                    "no materialized Section plane for {} on the Extract task",
                    task.arxiv_id
                ),
            };
        };
        match self.use_case.extract_from_parsed(&parsed).await {
            Ok(result) => SchedulerExecuteOutcome::Completed {
                summary: format!(
                    "extracted {} entities ({} created) for {}",
                    result.entities_extracted, result.entities_created, task.arxiv_id
                ),
                evidence_refs: Vec::new(),
            },
            Err(error) => SchedulerExecuteOutcome::Retryable {
                error_class: "extraction_failed".into(),
                message: format!("{}: {error:#}", task.arxiv_id),
            },
        }
    }
}

/// Executor for the `Ingest` task type: parse+embed+materialize a catalog PDF.
pub struct IngestSchedulerExecutor {
    locator: Arc<dyn CatalogPdfLocator>,
    use_case: IngestUseCase,
}

impl IngestSchedulerExecutor {
    pub fn new(
        locator: Arc<dyn CatalogPdfLocator>,
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Arc<dyn DirectGraphStore>,
    ) -> Self {
        let use_case = IngestUseCase::from_shared(parser, embedder, graph_store);
        Self { locator, use_case }
    }
}

#[async_trait]
impl SchedulerTaskExecutor for IngestSchedulerExecutor {
    fn task_type(&self) -> TaskType {
        TaskType::Ingest
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        let Some(pdf_path) = self.locator.locate(&task.arxiv_id) else {
            return SchedulerExecuteOutcome::Retryable {
                error_class: "catalog_pdf_not_found".into(),
                message: format!(
                    "no catalog source PDF for {} (locator miss, not invented)",
                    task.arxiv_id
                ),
            };
        };
        match self
            .use_case
            .ingest_pdf(pdf_path.to_str().unwrap_or_default(), &task.arxiv_id)
            .await
        {
            Ok(result) => SchedulerExecuteOutcome::Completed {
                summary: format!(
                    "ingested {} (sections={} body_chars={})",
                    result.paper_id, result.section_count, result.body_chars
                ),
                evidence_refs: Vec::new(),
            },
            Err(error) => SchedulerExecuteOutcome::Retryable {
                error_class: "ingest_failed".into(),
                message: format!("{}: {error:#}", task.arxiv_id),
            },
        }
    }
}

/// Executor for the `Reparse` task type: re-parse a catalog PDF for a Paper
/// that is already on the graph (parser/profile change, ADR-056).
pub struct ReparseSchedulerExecutor {
    locator: Arc<dyn CatalogPdfLocator>,
    graph_store: Arc<dyn DirectGraphStore>,
    use_case: IngestUseCase,
}

impl ReparseSchedulerExecutor {
    pub fn new(
        locator: Arc<dyn CatalogPdfLocator>,
        parser: Box<dyn ParserPort>,
        embedder: Box<dyn Embedder>,
        graph_store: Arc<dyn DirectGraphStore>,
    ) -> Self {
        let use_case = IngestUseCase::from_shared(parser, embedder, graph_store.clone());
        Self {
            locator,
            graph_store,
            use_case,
        }
    }
}

#[async_trait]
impl SchedulerTaskExecutor for ReparseSchedulerExecutor {
    fn task_type(&self) -> TaskType {
        TaskType::Reparse
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        let Some(pdf_path) = self.locator.locate(&task.arxiv_id) else {
            return SchedulerExecuteOutcome::Retryable {
                error_class: "catalog_pdf_not_found".into(),
                message: format!(
                    "no catalog source PDF for {} (locator miss, not invented)",
                    task.arxiv_id
                ),
            };
        };
        if self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", &task.arxiv_id)
            .await
            .is_none()
        {
            return SchedulerExecuteOutcome::Retryable {
                error_class: "paper_not_materialized".into(),
                message: format!(
                    "no Paper node for {} (Reparse is not first-time Ingest)",
                    task.arxiv_id
                ),
            };
        }
        match self
            .use_case
            .ingest_pdf(pdf_path.to_str().unwrap_or_default(), &task.arxiv_id)
            .await
        {
            Ok(result) => SchedulerExecuteOutcome::Completed {
                summary: format!(
                    "reparsed {} (sections={} body_chars={})",
                    result.paper_id, result.section_count, result.body_chars
                ),
                evidence_refs: Vec::new(),
            },
            Err(error) => SchedulerExecuteOutcome::Retryable {
                error_class: "reparse_failed".into(),
                message: format!("{}: {error:#}", task.arxiv_id),
            },
        }
    }
}

/// Executor for the `EmbedStale` task type: re-embed a materialized Paper
/// under a changed embedder model and apply a sanctioned same-node vector
/// replacement (ADR-037 / D277). Same-sha is a no-op.
pub struct EmbedStaleSchedulerExecutor {
    graph_store: Arc<dyn DirectGraphStore>,
    embedder: Box<dyn Embedder>,
}

impl EmbedStaleSchedulerExecutor {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, embedder: Box<dyn Embedder>) -> Self {
        Self {
            graph_store,
            embedder,
        }
    }

    async fn paper_embed_input(&self, arxiv_id: &str) -> Option<(u64, String)> {
        let paper = self
            .graph_store
            .find_node_by_string_property("Paper", "arxiv_id", arxiv_id)
            .await?;
        let title = self
            .graph_store
            .get_node_property_string(paper, "title")
            .await
            .unwrap_or_default();
        let abstract_text = self
            .graph_store
            .get_node_property_string(paper, "abstract_text")
            .await
            .unwrap_or_default();
        let text = if abstract_text.trim().is_empty() {
            title
        } else {
            format!("{title}\n{abstract_text}")
        };
        if text.trim().is_empty() {
            return None;
        }
        Some((paper, text))
    }
}

#[async_trait]
impl SchedulerTaskExecutor for EmbedStaleSchedulerExecutor {
    fn task_type(&self) -> TaskType {
        TaskType::EmbedStale
    }

    async fn execute(&self, task: &PendingTask) -> SchedulerExecuteOutcome {
        let Some((paper_node, text)) = self.paper_embed_input(&task.arxiv_id).await else {
            return SchedulerExecuteOutcome::Retryable {
                error_class: "paper_not_materialized".into(),
                message: format!(
                    "no embeddable Paper content for {} (EmbedStale needs a materialized Paper)",
                    task.arxiv_id
                ),
            };
        };
        let vector = match self.embedder.embed(&text).await {
            Ok(vector) => vector,
            Err(error) => {
                return SchedulerExecuteOutcome::Retryable {
                    error_class: "embedding_failed".into(),
                    message: format!("{}: {error:#}", task.arxiv_id),
                };
            }
        };
        let new_sha = crate::ingest::embedding_fingerprint(&vector);
        let existing_sha = self
            .graph_store
            .get_node_property_string(paper_node, "embedding_sha256")
            .await;
        match existing_sha {
            Some(existing) if existing == new_sha => SchedulerExecuteOutcome::Completed {
                summary: format!(
                    "embedding up to date for {} (sha {})",
                    task.arxiv_id, new_sha
                ),
                evidence_refs: Vec::new(),
            },
            Some(_existing) => {
                if let Err(error) = self
                    .graph_store
                    .replace_vector("Paper", "embedding", paper_node, vector.clone())
                    .await
                {
                    return SchedulerExecuteOutcome::Retryable {
                        error_class: "replace_vector_failed".into(),
                        message: format!("{}: {error}", task.arxiv_id),
                    };
                }
                if let Err(error) = self
                    .graph_store
                    .set_node_property_string(paper_node, "embedding_sha256", new_sha.clone())
                    .await
                {
                    return SchedulerExecuteOutcome::Retryable {
                        error_class: "embedding_sha_stamp_failed".into(),
                        message: format!("{}: {error}", task.arxiv_id),
                    };
                }
                SchedulerExecuteOutcome::Completed {
                    summary: format!(
                        "re-embedded {} under model {} ({})",
                        task.arxiv_id,
                        self.embedder.model_id(),
                        new_sha
                    ),
                    evidence_refs: Vec::new(),
                }
            }
            None => SchedulerExecuteOutcome::Retryable {
                error_class: "embedding_never_attached".into(),
                message: format!(
                    "no embedding on Paper {}; run Ingest/Extract journey to attach first",
                    task.arxiv_id
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::entity::EntityType;
    use da_ports::extractor::ExtractResult;
    use da_ports::extractor::ExtractedEntity;

    fn extract_pending_task(arxiv_id: &str) -> PendingTask {
        use da_domain::scheduler::{TaskPriority, TaskStatus};
        PendingTask {
            arxiv_id: arxiv_id.to_string(),
            task_type: TaskType::Extract,
            added_at: 0,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Medium,
            lease: None,
            dependencies: vec![],
            resource_class: da_domain::scheduler::TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        }
    }

    struct DummyExtractor;
    #[async_trait]
    impl Extractor for DummyExtractor {
        async fn extract(
            &self,
            sections: &[(String, String)],
        ) -> ExtractResult<Vec<ExtractedEntity>> {
            Ok(sections
                .iter()
                .enumerate()
                .map(|(i, (title, _))| ExtractedEntity {
                    label: format!("Entity{i}"),
                    entity_type: EntityType::Method,
                    section_title: title.clone(),
                    section_ordinal: Some(i),
                    char_start: 0,
                    char_end: 0,
                    surface: String::new(),
                })
                .collect())
        }
        fn name(&self) -> &str {
            "dummy"
        }
    }

    async fn seed_graph(
        store: &kg_storage::MockGraphStore,
        arxiv_id: &str,
        sections: &[(&str, &str)],
    ) {
        use da_ports::graph_store::DirectGraphStore as _;
        let paper = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(paper, "arxiv_id", arxiv_id.to_string())
            .await
            .unwrap();
        store
            .set_node_property_string(paper, "title", "Test Title".to_string())
            .await
            .unwrap();
        for (i, (title, text)) in sections.iter().enumerate() {
            let section = store.create_node("Section").await.unwrap();
            store
                .set_node_property_string(section, "title", (*title).to_string())
                .await
                .unwrap();
            store
                .set_node_property_string(section, "text", (*text).to_string())
                .await
                .unwrap();
            store
                .set_node_property_int(section, "order", i as i64)
                .await
                .unwrap();
            store
                .create_edge(paper, section, da_domain::relation::structure::HAS_PART)
                .await
                .unwrap();
        }
    }

    #[test]
    fn provider_rebuilds_parsed_from_graph_sections() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(&store, "2506.07853", &[("Intro", "long text")]));
        let provider = GraphParsedArticleProvider::new(std::sync::Arc::new(store));
        let parsed = rt.block_on(provider.provide("2506.07853")).expect("parsed");
        assert_eq!(parsed.paper_id, "2506.07853");
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].title, "Intro");
    }

    #[test]
    fn provider_returns_none_for_missing_paper() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(&store, "2506.07853", &[("Intro", "text")]));
        let provider = GraphParsedArticleProvider::new(std::sync::Arc::new(store));
        assert!(rt.block_on(provider.provide("9999.99999")).is_none());
    }

    #[test]
    fn extract_executor_completes_on_materialized_plane() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(
            &store,
            "2506.07853",
            &[("Intro", "methods"), ("Results", "accuracy")],
        ));
        let executor =
            ExtractSchedulerExecutor::new(std::sync::Arc::new(store), Box::new(DummyExtractor));
        let task = extract_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Completed { summary, .. } => {
                assert!(summary.contains("2 entities"), "{summary}");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn extract_executor_retries_when_plane_missing() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(&store, "2506.07853", &[("Intro", "text")]));
        let executor =
            ExtractSchedulerExecutor::new(std::sync::Arc::new(store), Box::new(DummyExtractor));
        let task = extract_pending_task("9999.99999");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Retryable { error_class, .. } => {
                assert_eq!(error_class, "parsed_plane_not_materialized")
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    fn ingest_pending_task(arxiv_id: &str) -> PendingTask {
        use da_domain::scheduler::{TaskPriority, TaskStatus};
        PendingTask {
            arxiv_id: arxiv_id.to_string(),
            task_type: TaskType::Ingest,
            added_at: 0,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::High,
            lease: None,
            dependencies: vec![],
            resource_class: da_domain::scheduler::TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        }
    }

    struct FixedLocator(Option<std::path::PathBuf>);
    impl CatalogPdfLocator for FixedLocator {
        fn locate(&self, _arxiv_id: &str) -> Option<std::path::PathBuf> {
            self.0.clone()
        }
    }

    struct DummyParser;
    #[async_trait]
    impl ParserPort for DummyParser {
        async fn parse_pdf(
            &self,
            _pdf_path: &str,
            paper_id: &str,
        ) -> da_ports::parser::ParseResult<ParsedArticle> {
            Ok(ParsedArticle {
                paper_id: paper_id.to_string(),
                title: format!("Title {paper_id}"),
                abstract_text: "abs".into(),
                body_text: "x".repeat(32),
                sections: vec![Section {
                    title: "Intro".into(),
                    text: "body".into(),
                    level: 1,
                    ..Default::default()
                }],
                citations: Vec::new(),
                layout_json: None,
                tei_xml: None,
                tables: Vec::new(),
                table_missingness: None,
                formulas: Vec::new(),
                formula_missingness: None,
                pdf_hash: "hash".into(),
            })
        }
    }

    struct DummyEmbedder;
    #[async_trait]
    impl da_ports::embedder::Embedder for DummyEmbedder {
        async fn embed(&self, _text: &str) -> da_ports::embedder::EmbedResult<Vec<f32>> {
            Ok(vec![0.1; 8])
        }
        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> da_ports::embedder::EmbedResult<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1; 8]).collect())
        }
        fn dimensions(&self) -> usize {
            8
        }
        fn model_id(&self) -> &str {
            "dummy"
        }
    }

    #[test]
    fn ingest_executor_retries_when_catalog_pdf_missing() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        let executor = IngestSchedulerExecutor::new(
            std::sync::Arc::new(FixedLocator(None)),
            Box::new(DummyParser),
            Box::new(DummyEmbedder),
            std::sync::Arc::new(store),
        );
        let task = ingest_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Retryable { error_class, .. } => {
                assert_eq!(error_class, "catalog_pdf_not_found");
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    #[test]
    fn ingest_executor_completes_on_locator_hit() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("source").join("2506.07853.pdf");
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        std::fs::write(&pdf, b"%PDF-1.4 test").unwrap();
        let store = kg_storage::MockGraphStore::new();
        let executor = IngestSchedulerExecutor::new(
            std::sync::Arc::new(FixedLocator(Some(pdf))),
            Box::new(DummyParser),
            Box::new(DummyEmbedder),
            std::sync::Arc::new(store),
        );
        let task = ingest_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Completed { summary, .. } => {
                assert!(summary.contains("2506.07853"), "{summary}");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    fn reparse_pending_task(arxiv_id: &str) -> PendingTask {
        use da_domain::scheduler::{TaskPriority, TaskStatus};
        PendingTask {
            arxiv_id: arxiv_id.to_string(),
            task_type: TaskType::Reparse,
            added_at: 0,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::High,
            lease: None,
            dependencies: vec![],
            resource_class: da_domain::scheduler::TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        }
    }

    fn reparse_executor(
        locator: FixedLocator,
        store: kg_storage::MockGraphStore,
    ) -> ReparseSchedulerExecutor {
        ReparseSchedulerExecutor::new(
            std::sync::Arc::new(locator),
            Box::new(DummyParser),
            Box::new(DummyEmbedder),
            std::sync::Arc::new(store),
        )
    }

    #[test]
    fn reparse_executor_retries_when_catalog_pdf_missing() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(&store, "2506.07853", &[("Intro", "body")]));
        let executor = reparse_executor(FixedLocator(None), store);
        let task = reparse_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Retryable { error_class, .. } => {
                assert_eq!(error_class, "catalog_pdf_not_found");
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    #[test]
    fn reparse_executor_retries_when_paper_missing() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("source").join("2506.07853.pdf");
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        std::fs::write(&pdf, b"%PDF-1.4 test").unwrap();
        let store = kg_storage::MockGraphStore::new();
        let executor = reparse_executor(FixedLocator(Some(pdf)), store);
        let task = reparse_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Retryable { error_class, .. } => {
                assert_eq!(error_class, "paper_not_materialized");
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    #[test]
    fn reparse_executor_completes_on_locator_hit_and_existing_paper() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("source").join("2506.07853.pdf");
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        std::fs::write(&pdf, b"%PDF-1.4 test").unwrap();
        let store = kg_storage::MockGraphStore::new();
        rt.block_on(seed_graph(&store, "2506.07853", &[("Intro", "body")]));
        let executor = reparse_executor(FixedLocator(Some(pdf)), store);
        let task = reparse_pending_task("2506.07853");
        match rt.block_on(executor.execute(&task)) {
            SchedulerExecuteOutcome::Completed { summary, .. } => {
                assert!(summary.contains("reparsed"), "{summary}");
                assert!(summary.contains("2506.07853"), "{summary}");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    fn embed_stale_pending_task(arxiv_id: &str) -> PendingTask {
        use da_domain::scheduler::{TaskPriority, TaskStatus};
        PendingTask {
            arxiv_id: arxiv_id.to_string(),
            task_type: TaskType::EmbedStale,
            added_at: 0,
            retry_count: 0,
            last_retry: None,
            next_retry: 0,
            status: TaskStatus::Pending,
            schema_version: 2,
            priority: TaskPriority::Low,
            lease: None,
            dependencies: vec![],
            resource_class: da_domain::scheduler::TaskResourceClass::Standard,
            attempts: vec![],
            logical_result: None,
        }
    }

    async fn seed_embeddable_paper(store: &kg_storage::MockGraphStore, arxiv_id: &str) -> u64 {
        use da_ports::graph_store::DirectGraphStore as _;
        let paper = store.create_node("Paper").await.unwrap();
        store
            .set_node_property_string(paper, "arxiv_id", arxiv_id.to_string())
            .await
            .unwrap();
        store
            .set_node_property_string(paper, "title", "ReEmbed Test Title".to_string())
            .await
            .unwrap();
        store
            .set_node_property_string(
                paper,
                "abstract_text",
                "some abstract for re-embedding".to_string(),
            )
            .await
            .unwrap();
        paper
    }

    struct FixedVecEmbedder {
        vector: Vec<f32>,
    }
    #[async_trait]
    impl da_ports::embedder::Embedder for FixedVecEmbedder {
        async fn embed(&self, _text: &str) -> da_ports::embedder::EmbedResult<Vec<f32>> {
            Ok(self.vector.clone())
        }
        async fn embed_batch(
            &self,
            _texts: &[&str],
        ) -> da_ports::embedder::EmbedResult<Vec<Vec<f32>>> {
            Ok(vec![])
        }
        fn dimensions(&self) -> usize {
            self.vector.len()
        }
        fn model_id(&self) -> &str {
            "fixed-model"
        }
    }

    #[test]
    fn embed_stale_executor_replaces_vector_on_conflict_and_noops_on_same_sha() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        // Seed with vector A and its sha.
        let a = vec![0.1, 0.2];
        let b = vec![7.0, 8.0];
        let sha_a = crate::ingest::embedding_fingerprint(&a);
        let store = kg_storage::MockGraphStore::new();
        let paper = rt.block_on(seed_embeddable_paper(&store, "2506.07853"));
        rt.block_on(store.add_vector("Paper", "embedding", paper, a.clone()));
        rt.block_on(store.set_node_property_string(paper, "embedding_sha256", sha_a.clone()));
        let task = embed_stale_pending_task("2506.07853");

        // Same embedder output -> no-op Completed, sha unchanged.
        let noop = EmbedStaleSchedulerExecutor::new(
            std::sync::Arc::new(store.clone()),
            Box::new(FixedVecEmbedder { vector: a.clone() }),
        );
        match rt.block_on(noop.execute(&task)) {
            SchedulerExecuteOutcome::Completed { summary, .. } => {
                assert!(summary.contains("up to date"), "{summary}");
            }
            other => panic!("expected no-op Completed, got {other:?}"),
        }
        assert_eq!(
            rt.block_on(store.get_node_property_string(paper, "embedding_sha256")),
            Some(sha_a.clone())
        );

        // Changed embedder output -> sanctioned same-node replace, sha advances.
        let replace = EmbedStaleSchedulerExecutor::new(
            std::sync::Arc::new(store.clone()),
            Box::new(FixedVecEmbedder { vector: b.clone() }),
        );
        match rt.block_on(replace.execute(&task)) {
            SchedulerExecuteOutcome::Completed { summary, .. } => {
                assert!(summary.contains("re-embedded"), "{summary}");
            }
            other => panic!("expected replace Completed, got {other:?}"),
        }
        let sha_b = crate::ingest::embedding_fingerprint(&b);
        assert_eq!(
            rt.block_on(store.get_node_property_string(paper, "embedding_sha256")),
            Some(sha_b.clone())
        );
        let _ = sha_b;
    }
}
