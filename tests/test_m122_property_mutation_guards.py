from __future__ import annotations

from dataclasses import dataclass

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.application.corpus.coverage import (
    CatalogCoverageInput,
    CorpusCoverageRequest,
    CorpusCoverageUseCase,
    GraphProbeCoverageInput,
    ParserCoverageInput,
)
from research_graph.application.graph.probe import (
    GraphProbeArticleEvidence,
    GraphProbeExcludedRecord,
    GraphProbeExecutionResult,
    GraphProbeMetrics,
    GraphProbeRequest,
    GraphProbeUseCase,
)


@st.composite
def coverage_requests(draw: st.DrawFn) -> CorpusCoverageRequest:
    completed = draw(st.integers(min_value=0, max_value=40))
    skipped = draw(st.integers(min_value=0, max_value=8))
    errors = draw(st.integers(min_value=0, max_value=4))
    total = completed + skipped + errors
    index_entries = draw(st.one_of(st.none(), st.integers(min_value=total, max_value=80)))
    m056_records = draw(st.integers(min_value=0, max_value=80))
    html_count = draw(st.integers(min_value=0, max_value=completed))
    pdf_count = completed - html_count
    skip_reason_counts = (
        {"metadata_only_no_local_source_artifact": skipped} if skipped else {}
    )
    skipped_refs = tuple(f"metadata-only/{index}" for index in reversed(range(skipped)))
    graph = draw(st.booleans())
    graph_probe = None
    if graph:
        graph_probe = GraphProbeCoverageInput(
            corpus_size=completed,
            skipped_metadata_only=skipped,
            chunk_count_total=draw(st.integers(min_value=completed, max_value=completed + 200)),
            n_nodes=draw(st.integers(min_value=completed, max_value=completed + 500)),
            n_edges=draw(st.integers(min_value=0, max_value=1000)),
            citation_relations_count=draw(st.integers(min_value=0, max_value=500)),
            peak_memory_mb=draw(st.floats(min_value=0, max_value=100, allow_nan=False)),
        )
    return CorpusCoverageRequest(
        corpus_id="property-corpus",
        catalog=CatalogCoverageInput(
            total_records=m056_records,
            index_entries=index_entries,
            ingested_count=m056_records,
        ),
        parser=ParserCoverageInput(
            total=total,
            completed=completed,
            skipped=skipped,
            errors=errors,
            chunk_count_total=draw(st.integers(min_value=0, max_value=500)),
            source_kind_counts={"html_native": html_count, "pdf_converted": pdf_count},
            skip_reason_counts=skip_reason_counts,
            skipped_article_refs=skipped_refs,
        ),
        graph_probe=graph_probe,
    )


@given(request=coverage_requests())
@settings(max_examples=75)
def test_coverage_use_case_property_preserves_denominators_and_status(
    request: CorpusCoverageRequest,
) -> None:
    result = CorpusCoverageUseCase().run(request)
    denominators = {denominator.name: denominator for denominator in result.denominators}
    expected_catalog_records = request.catalog.index_entries or request.catalog.total_records

    assert result.catalog_records == expected_catalog_records
    assert result.parser_total == request.parser.total
    assert result.parser_completed == request.parser.completed
    assert result.parser_skipped == request.parser.skipped
    assert result.parser_errors == request.parser.errors
    assert result.source_backed_records == request.parser.completed
    assert result.metadata_only_records == request.parser.skipped
    assert result.succeeded is (request.parser.errors == 0)

    assert denominators["catalog_articles"].total == expected_catalog_records
    assert denominators["catalog_articles"].included == expected_catalog_records
    assert denominators["parser_replay_articles"].total == request.parser.total
    assert denominators["parser_replay_articles"].included == request.parser.completed
    assert denominators["parser_replay_articles"].excluded == request.parser.skipped
    assert denominators["parser_replay_articles"].errors == request.parser.errors
    assert denominators["source_backed_articles"].total == (
        request.parser.completed + request.parser.skipped
    )
    assert denominators["source_backed_articles"].included == request.parser.completed
    assert denominators["source_backed_articles"].excluded == request.parser.skipped

    assert result.skipped_article_refs == tuple(sorted(request.parser.skipped_article_refs))
    assert sum(result.source_kind_counts.values()) == request.parser.completed
    if request.parser.skipped:
        diagnostics = {diagnostic.code: diagnostic for diagnostic in result.diagnostics}
        assert diagnostics["metadata_only_no_local_source_artifact"].count == request.parser.skipped
    if request.parser.errors:
        diagnostics = {diagnostic.code: diagnostic for diagnostic in result.diagnostics}
        assert diagnostics["parser_errors"].count == request.parser.errors


@dataclass
class RecordingExecutor:
    metrics: GraphProbeMetrics | None = None
    calls: int = 0

    def execute(self, request: GraphProbeRequest) -> GraphProbeExecutionResult:
        self.calls += 1
        return GraphProbeExecutionResult(
            metrics=self.metrics
            or GraphProbeMetrics(
                n_nodes=sum(article.chunk_count for article in request.completed_articles)
                + len(request.completed_articles)
                + len(request.excluded_records),
                n_edges=sum(article.chunk_count for article in request.completed_articles),
                node_types={"article": len(request.completed_articles)},
                edge_types={"article_contains_chunk": sum(article.chunk_count for article in request.completed_articles)},
                citation_relations_count=0,
            )
        )


@st.composite
def graph_requests(draw: st.DrawFn) -> GraphProbeRequest:
    count = draw(st.integers(min_value=0, max_value=10))
    skipped = draw(st.integers(min_value=0, max_value=4))
    articles = tuple(
        GraphProbeArticleEvidence(
            article_ref=f"arxiv/cs-cl/{index}",
            article_key=f"{index}",
            chunk_count=draw(st.integers(min_value=0, max_value=12)),
            source_kind=draw(st.sampled_from(["html_native", "pdf_converted", "unknown"])),
        )
        for index in range(count)
    )
    excluded = tuple(
        GraphProbeExcludedRecord(
            article_ref=f"metadata-only/{index}",
            article_key=f"metadata-{index}",
            skip_reason="metadata_only_no_local_source_artifact",
        )
        for index in range(skipped)
    )
    return GraphProbeRequest(
        corpus_id="property-graph",
        completed_articles=articles,
        excluded_records=excluded,
    )


@given(request=graph_requests())
@settings(max_examples=75)
def test_graph_probe_use_case_property_preserves_accounting_and_calls_adapter_once(
    request: GraphProbeRequest,
) -> None:
    executor = RecordingExecutor()

    result = GraphProbeUseCase().run(request, executor)

    assert executor.calls == 1
    assert result.succeeded is True
    assert result.corpus_size == len(request.completed_articles)
    assert result.skipped_metadata_only == len(request.excluded_records)
    assert result.total_catalog_records_seen == (
        len(request.completed_articles) + len(request.excluded_records)
    )
    assert result.chunk_count_total == sum(article.chunk_count for article in request.completed_articles)
    assert result.completed_articles == tuple(request.completed_articles)
    assert result.excluded_records == tuple(request.excluded_records)
    assert all(value is False for value in result.fail_closed_invariants.values())
    assert result.metrics is not None
    assert result.metrics.n_edges == result.chunk_count_total


@pytest.mark.parametrize("chunk_count", [-1, -5])
def test_graph_probe_use_case_mutation_guard_negative_chunk_counts_skip_adapter(
    chunk_count: int,
) -> None:
    request = GraphProbeRequest(
        corpus_id="property-graph",
        completed_articles=(
            GraphProbeArticleEvidence(
                article_ref="arxiv/cs-cl/bad",
                article_key="bad",
                chunk_count=chunk_count,
            ),
        ),
    )
    executor = RecordingExecutor()

    result = GraphProbeUseCase().run(request, executor)

    assert executor.calls == 0
    assert result.succeeded is False
    assert result.first_failure_code == "invalid_chunk_count"
    assert result.failure_phase == "input_validation"
    assert result.diagnostics[0].count == 1
