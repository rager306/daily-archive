//! Honest shared-store execution for due OpenAlex scheduler tasks.

use crate::{EnrichUseCase, GraphScheduler};
use da_domain::scheduler::RetryPolicy;
use da_ports::graph_store::DirectGraphStore;
use da_ports::openalex::OpenAlexClient;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerTaskDisposition {
    Completed,
    Pending,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SchedulerTaskOutcome {
    pub node_id: u64,
    pub arxiv_id: String,
    pub disposition: SchedulerTaskDisposition,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerRunReport {
    pub due: usize,
    pub completed: usize,
    pub pending: usize,
    pub failed: usize,
    pub outcomes: Vec<SchedulerTaskOutcome>,
}

pub struct SchedulerRunner {
    graph_store: Arc<dyn DirectGraphStore>,
    openalex: Arc<dyn OpenAlexClient>,
    policy: RetryPolicy,
    clock: Arc<dyn da_ports::Clock>,
}

impl SchedulerRunner {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>, openalex: Arc<dyn OpenAlexClient>) -> Self {
        Self {
            graph_store,
            openalex,
            policy: RetryPolicy::default(),
            clock: Arc::new(da_ports::SystemClock),
        }
    }

    pub fn with_clock(mut self, clock: Arc<dyn da_ports::Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub async fn run_due(&self) -> anyhow::Result<SchedulerRunReport> {
        let now = self.clock.now().timestamp();
        let due = GraphScheduler::load_due_tasks_from(self.graph_store.as_ref(), now).await;
        let mut report = SchedulerRunReport {
            due: due.len(),
            ..Default::default()
        };
        for (node_id, arxiv_id) in due {
            let use_case = EnrichUseCase::from_shared(
                Arc::clone(&self.openalex),
                Arc::clone(&self.graph_store),
            )
            .with_clock(Arc::clone(&self.clock));
            match use_case.enrich_by_arxiv_id(&arxiv_id).await {
                Ok(result) if result.openalex_pending => {
                    GraphScheduler::record_retry_on(
                        self.graph_store.as_ref(),
                        &self.policy,
                        node_id,
                        now,
                    )
                    .await?;
                    let disposition = self.current_retry_disposition(node_id).await;
                    if disposition == SchedulerTaskDisposition::Failed {
                        report.failed += 1;
                    } else {
                        report.pending += 1;
                    }
                    report.outcomes.push(SchedulerTaskOutcome {
                        node_id,
                        arxiv_id,
                        disposition,
                        message: "OpenAlex work not found; retry lifecycle advanced".into(),
                    });
                }
                Ok(result) => {
                    let written_identity = match self
                        .graph_store
                        .find_node_by_string_property("Paper", "arxiv_id", &arxiv_id)
                        .await
                    {
                        Some(paper) => {
                            self.graph_store
                                .get_node_property_string(paper, "openalex_id")
                                .await
                        }
                        None => None,
                    };
                    if written_identity.as_deref() != Some(result.openalex_id.as_str())
                        || result.openalex_id.is_empty()
                    {
                        GraphScheduler::record_retry_on(
                            self.graph_store.as_ref(),
                            &self.policy,
                            node_id,
                            now,
                        )
                        .await?;
                        let disposition = self.current_retry_disposition(node_id).await;
                        if disposition == SchedulerTaskDisposition::Failed {
                            report.failed += 1;
                        } else {
                            report.pending += 1;
                        }
                        report.outcomes.push(SchedulerTaskOutcome {
                            node_id,
                            arxiv_id,
                            disposition,
                            message: "Paper lacks verified OpenAlex write-back; completion refused"
                                .into(),
                        });
                        continue;
                    }
                    GraphScheduler::complete_task_on(self.graph_store.as_ref(), node_id, now)
                        .await?;
                    report.completed += 1;
                    report.outcomes.push(SchedulerTaskOutcome {
                        node_id,
                        arxiv_id,
                        disposition: SchedulerTaskDisposition::Completed,
                        message: format!(
                            "{} topics, {} authors, {} citations",
                            result.topics_written,
                            result.authors_written,
                            result.cites_edges_written
                        ),
                    });
                }
                Err(error) => {
                    GraphScheduler::record_retry_on(
                        self.graph_store.as_ref(),
                        &self.policy,
                        node_id,
                        now,
                    )
                    .await?;
                    let disposition = self.current_retry_disposition(node_id).await;
                    if disposition == SchedulerTaskDisposition::Failed {
                        report.failed += 1;
                    } else {
                        report.pending += 1;
                    }
                    report.outcomes.push(SchedulerTaskOutcome {
                        node_id,
                        arxiv_id,
                        disposition,
                        message: format!("OpenAlex enrichment error: {error:#}"),
                    });
                }
            }
        }
        tracing::info!(
            due = report.due,
            completed = report.completed,
            pending = report.pending,
            failed = report.failed,
            "Scheduler run completed on shared graph store"
        );
        Ok(report)
    }

    async fn current_retry_disposition(&self, node_id: u64) -> SchedulerTaskDisposition {
        match self
            .graph_store
            .get_node_property_string(node_id, "status")
            .await
            .as_deref()
        {
            Some("failed") => SchedulerTaskDisposition::Failed,
            _ => SchedulerTaskDisposition::Pending,
        }
    }
}
