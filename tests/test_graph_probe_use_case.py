from __future__ import annotations

from dataclasses import dataclass

from research_graph.application.graph.probe import (
    GraphProbeArticleEvidence,
    GraphProbeArtifactRef,
    GraphProbeDiagnostic,
    GraphProbeExcludedRecord,
    GraphProbeExecutionResult,
    GraphProbeMemoryProfile,
    GraphProbeMetrics,
    GraphProbeRequest,
    GraphProbeUseCase,
)


@dataclass
class StubExecutor:
    result: GraphProbeExecutionResult
    calls: int = 0

    def execute(self, request: GraphProbeRequest) -> GraphProbeExecutionResult:
        self.calls += 1
        assert request.corpus_id == "r024-fixture"
        return self.result


class ExplodingExecutor:
    def execute(self, request: GraphProbeRequest) -> GraphProbeExecutionResult:
        raise RuntimeError("corpus body text must not leak")


def _request() -> GraphProbeRequest:
    return GraphProbeRequest(
        corpus_id="r024-fixture",
        completed_articles=(
            GraphProbeArticleEvidence(
                article_ref="arxiv/cs-cl/2605.18747",
                article_key="2605.18747",
                chunk_count=3,
                source_kind="pdf_converted",
                text_source="data/article.md",
            ),
            GraphProbeArticleEvidence(
                article_ref="company_blog/example",
                article_key="example",
                chunk_count=2,
                source_kind="html_native",
                text_source="data/example.md",
            ),
        ),
        excluded_records=(
            GraphProbeExcludedRecord(
                article_ref="stanford/cs224n/gradient-notes",
                article_key="gradient-notes",
                skip_reason="metadata_only_no_local_source_artifact",
            ),
        ),
        input_artifacts=(
            GraphProbeArtifactRef(
                path="data/r024-fixture/parser-chunking/events.jsonl",
                artifact_type="parser-events",
                schema_version="parser-replay-events.v00.01",
            ),
        ),
        entity_types=("metadata", "citation_context"),
    )


def test_graph_probe_use_case_returns_accounting_and_metrics() -> None:
    executor = StubExecutor(
        result=GraphProbeExecutionResult(
            metrics=GraphProbeMetrics(
                n_nodes=12,
                n_edges=11,
                node_types={"article": 2, "chunk": 5, "corpus": 1, "entity": 4},
                edge_types={
                    "corpus_contains_article": 2,
                    "article_contains_chunk": 5,
                    "article_has_entity": 4,
                },
                citation_relations_count=0,
            ),
            memory_profile=GraphProbeMemoryProfile(
                method="tracemalloc",
                peak_mb=0.25,
                current_mb=0.1,
                approx_bytes_per_node=1024,
            ),
            implementation={
                "library": "networkx",
                "graph_type": "DiGraph",
                "in_memory_only": True,
                "no_db_connection": True,
                "no_network_io": True,
            },
            output_artifacts=(
                GraphProbeArtifactRef(
                    path="data/r024-fixture/networkx-probe/summary.json",
                    artifact_type="json-summary",
                    schema_version="r024-networkx-probe-summary.v00.01",
                ),
            ),
        )
    )

    result = GraphProbeUseCase().run(_request(), executor)

    assert executor.calls == 1
    assert result.succeeded is True
    assert result.schema_version == "graph-probe-result.v00.01"
    assert result.total_catalog_records_seen == 3
    assert result.corpus_size == 2
    assert result.skipped_metadata_only == 1
    assert result.chunk_count_total == 5
    assert result.source_kind_counts == {"html_native": 1, "pdf_converted": 1}
    assert result.metrics is not None
    assert result.metrics.n_nodes == 12
    assert result.metrics.n_edges == 11
    assert result.metrics.node_types["chunk"] == 5
    assert result.memory_profile is not None
    assert result.memory_profile.peak_mb == 0.25
    assert result.implementation["library"] == "networkx"
    assert result.input_artifacts[0].path.endswith("events.jsonl")
    assert result.output_artifacts[0].artifact_type == "json-summary"
    assert result.excluded_records[0].skip_reason == "metadata_only_no_local_source_artifact"
    assert all(value is False for value in result.fail_closed_invariants.values())


def test_graph_probe_use_case_warning_diagnostics_do_not_fail_successful_metrics() -> None:
    executor = StubExecutor(
        result=GraphProbeExecutionResult(
            metrics=GraphProbeMetrics(n_nodes=1, n_edges=0),
            diagnostics=(
                GraphProbeDiagnostic(
                    code="non_blocking_note",
                    phase="adapter_execution",
                    severity="warning",
                    notes="diagnostic without failure",
                ),
            ),
        )
    )

    result = GraphProbeUseCase().run(_request(), executor)

    assert executor.calls == 1
    assert result.succeeded is True
    assert result.failure_phase is None
    assert result.first_failure_code is None
    assert result.diagnostics[0].severity == "warning"


def test_graph_probe_use_case_rejects_invalid_chunk_counts_without_calling_adapter() -> None:
    request = GraphProbeRequest(
        corpus_id="r024-fixture",
        completed_articles=(
            GraphProbeArticleEvidence(
                article_ref="arxiv/cs-cl/2605.18747",
                article_key="2605.18747",
                chunk_count=-1,
            ),
        ),
    )
    executor = StubExecutor(result=GraphProbeExecutionResult())

    result = GraphProbeUseCase().run(request, executor)

    assert executor.calls == 0
    assert result.succeeded is False
    assert result.failure_phase == "input_validation"
    assert result.first_failure_code == "invalid_chunk_count"
    assert result.diagnostics[0].count == 1


def test_graph_probe_use_case_sanitizes_adapter_exceptions() -> None:
    result = GraphProbeUseCase().run(_request(), ExplodingExecutor())

    assert result.succeeded is False
    assert result.metrics is None
    assert result.failure_phase == "adapter_execution"
    assert result.first_failure_code == "adapter_exception"
    assert result.diagnostics[0].exception_class == "RuntimeError"
    assert "corpus body text" not in result.diagnostics[0].notes
    assert result.diagnostics[0].notes == "graph probe adapter raised before producing metrics"
