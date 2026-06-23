from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.graph.probe import (
    GraphProbeArticleEvidence,
    GraphProbeRequest,
)
from research_graph.infrastructure.graph.networkx_probe import (
    NetworkXGraphProbeAdapter,
    find_citation_relations,
)


class MissingNetworkX:
    pass


class BrokenTraceModule:
    def start(self) -> None:
        raise RuntimeError("trace unavailable")

    def stop(self) -> None:
        return None


class BrokenMetricsGraph:
    def add_node(self, *args: object, **kwargs: object) -> None:
        return None

    def add_edge(self, *args: object, **kwargs: object) -> None:
        return None

    def number_of_nodes(self) -> int:
        raise RuntimeError("node metric failed with payload")


class BrokenMetricsNetworkX:
    @staticmethod
    def DiGraph(name: str) -> BrokenMetricsGraph:
        return BrokenMetricsGraph()


def _fixture_request() -> GraphProbeRequest:
    return GraphProbeRequest(
        corpus_id="r024-fixture",
        completed_articles=(
            GraphProbeArticleEvidence(
                article_ref="arxiv/cs-cl/2605.18747",
                article_key="2605.18747",
                chunk_count=2,
                source_kind="pdf_converted",
                text_source="data/2605.18747.md",
            ),
            GraphProbeArticleEvidence(
                article_ref="arxiv/cs-cl/2605.18748",
                article_key="2605.18748",
                chunk_count=1,
                source_kind="pdf_converted",
                text_source="data/2605.18748.md",
            ),
            GraphProbeArticleEvidence(
                article_ref="company_blog/example",
                article_key="example",
                chunk_count=0,
                source_kind="html_native",
                text_source="data/example.md",
            ),
        ),
        entity_types=("metadata", "citation_context"),
    )


def test_networkx_adapter_extracts_fixture_metrics(tmp_path: Path) -> None:
    pytest.importorskip("networkx")
    graphml = tmp_path / "probe.graphml"

    result = NetworkXGraphProbeAdapter(graphml_path=graphml).execute(_fixture_request())

    assert result.succeeded is True
    assert result.metrics is not None
    assert result.metrics.n_nodes == 13
    assert result.metrics.n_edges == 13
    assert result.metrics.node_types == {
        "article": 3,
        "chunk": 3,
        "corpus": 1,
        "entity": 6,
    }
    assert result.metrics.edge_types == {
        "article_cites_article": 1,
        "article_contains_chunk": 3,
        "article_has_entity": 6,
        "corpus_contains_article": 3,
    }
    assert result.metrics.citation_relations_count == 1
    assert result.memory_profile is not None
    assert result.memory_profile.method == "tracemalloc"
    assert result.memory_profile.n_nodes == 13
    assert result.memory_profile.n_edges == 13
    assert result.memory_profile.peak_mb >= 0
    assert result.implementation == {
        "library": "networkx",
        "graph_type": "DiGraph",
        "in_memory_only": True,
        "no_db_connection": True,
        "no_network_io": True,
    }
    assert graphml.exists()
    assert result.output_artifacts[0].path == graphml.as_posix()
    assert result.output_artifacts[0].artifact_type == "graphml"


def test_find_citation_relations_groups_by_coarse_category() -> None:
    relations = find_citation_relations(
        [
            "arxiv/cs-cl/1",
            "arxiv/cs-cl/2",
            "arxiv/cs-cv/3",
            "company_blog/vendor/4",
            "company_blog/vendor/5",
        ]
    )

    assert relations == [
        ("arxiv/cs-cl/1", "arxiv/cs-cl/2", "cs-cl"),
        ("company_blog/vendor/4", "company_blog/vendor/5", "vendor"),
    ]


def test_networkx_adapter_reports_missing_dependency() -> None:
    result = NetworkXGraphProbeAdapter(graph_library=None).execute(_fixture_request())
    pytest.importorskip("networkx")
    assert result.succeeded is True


def test_networkx_adapter_reports_injected_missing_dependency() -> None:
    adapter = NetworkXGraphProbeAdapter(graph_library=MissingNetworkX())

    result = adapter.execute(_fixture_request())

    assert result.succeeded is False
    assert result.diagnostics[0].code == "graph_construction_failed"
    assert result.diagnostics[0].phase == "graph_construction"
    assert result.diagnostics[0].exception_class == "AttributeError"
    assert "payload" not in result.diagnostics[0].notes


def test_networkx_adapter_reports_memory_sampling_warning() -> None:
    pytest.importorskip("networkx")
    adapter = NetworkXGraphProbeAdapter(trace_module=BrokenTraceModule())

    result = adapter.execute(_fixture_request())

    assert result.succeeded is True
    assert result.memory_profile is None
    assert result.diagnostics[0].severity == "warning"
    assert result.diagnostics[0].code == "memory_sampling_start_failed"
    assert result.diagnostics[0].phase == "memory_sampling"


def test_networkx_adapter_reports_metric_extraction_failure_without_payload() -> None:
    adapter = NetworkXGraphProbeAdapter(graph_library=BrokenMetricsNetworkX())

    result = adapter.execute(_fixture_request())

    assert result.succeeded is False
    assert result.diagnostics[-1].code == "metric_extraction_failed"
    assert result.diagnostics[-1].phase == "metric_extraction"
    assert result.diagnostics[-1].exception_class == "RuntimeError"
    assert "payload" not in result.diagnostics[-1].notes
