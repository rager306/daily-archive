"""Tests for M121 S05: NetworkX probe over 219 source-backed records."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from research_graph.infrastructure.graph.r024_networkx_probe import (
    R024NetworkXProbeConfig,
    build_request,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
R218_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1"
PROBE_DIR = R218_DIR / "networkx-probe"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
MEMORY_PROFILE = PROBE_DIR / "memory-profile.json"
EVENTS_LOG = PROBE_DIR / "events.jsonl"
PARSER_EVENTS = R218_DIR / "parser-chunking" / "events.jsonl"
EXPECTED_COMPLETED = 219
EXPECTED_SKIPPED = 2
EXPECTED_CHUNKS = 2576
EXPECTED_ENTITIES = EXPECTED_COMPLETED * 5


def _load(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text())
    return data if isinstance(data, dict) else {}


def _events(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def test_probe_dir_and_files_exist() -> None:
    assert PROBE_DIR.exists()
    assert GRAPHML.exists()
    assert SUMMARY.exists()
    assert MEMORY_PROFILE.exists()
    assert EVENTS_LOG.exists()


def test_summary_fail_closed() -> None:
    summary = _load(SUMMARY)
    invariants = summary["fail_closed_invariants"]
    for key in (
        "network_fetch_attempted",
        "production_import_attempted",
        "graph_import_allowed",
        "ladybugdb_written",
        "trusted_kg_import_allowed",
        "graph_readiness_claim",
        "falkordb_written",
        "neo4j_written",
        "ladybugdb_connection_attempted",
    ):
        assert invariants[key] is False


def test_summary_implementation_in_memory_only() -> None:
    implementation = _load(SUMMARY)["implementation"]
    assert implementation["library"] == "networkx"
    assert implementation["graph_type"] == "DiGraph"
    assert implementation["in_memory_only"] is True
    assert implementation["no_db_connection"] is True
    assert implementation["no_network_io"] is True


def test_summary_counts_completed_and_excluded_records() -> None:
    summary = _load(SUMMARY)
    assert summary["total_catalog_records_seen"] == EXPECTED_COMPLETED + EXPECTED_SKIPPED
    assert summary["corpus_size"] == EXPECTED_COMPLETED
    assert summary["skipped_metadata_only"] == EXPECTED_SKIPPED
    assert summary["chunk_count_total"] == EXPECTED_CHUNKS
    assert summary["source_kind_counts"] == {"html_native": 21, "pdf_converted": 198}


def test_summary_excluded_records_are_metadata_only_boundary() -> None:
    excluded = _load(SUMMARY)["excluded_records"]
    refs = {item["article_ref"] for item in excluded}
    assert refs == {
        "arxiv/mixed-source/2605.29548",
        "stanford/cs224n/gradient-notes",
    }
    assert {item["skip_reason"] for item in excluded} == {"metadata_only_no_local_source_artifact"}


def test_summary_node_and_edge_types() -> None:
    summary = _load(SUMMARY)
    assert summary["n_nodes"] == 3891
    assert summary["n_edges"] == 10102
    assert summary["node_types"] == {
        "article": EXPECTED_COMPLETED,
        "chunk": EXPECTED_CHUNKS,
        "corpus": 1,
        "entity": EXPECTED_ENTITIES,
    }
    assert summary["edge_types"] == {
        "article_cites_article": 6212,
        "article_contains_chunk": EXPECTED_CHUNKS,
        "article_has_entity": EXPECTED_ENTITIES,
        "corpus_contains_article": EXPECTED_COMPLETED,
    }
    assert summary["citation_relations_count"] == 6212


def test_graphml_loadable_and_counts_match_summary() -> None:
    nx = pytest.importorskip("networkx")
    graph = nx.read_graphml(str(GRAPHML))
    summary = _load(SUMMARY)
    assert graph.number_of_nodes() == summary["n_nodes"]
    assert graph.number_of_edges() == summary["n_edges"]


def test_memory_profile_reasonable() -> None:
    profile = _load(MEMORY_PROFILE)
    assert profile["method"] == "tracemalloc"
    assert profile["n_nodes"] == 3891
    assert profile["n_edges"] == 10102
    assert float(profile["peak_mb"]) < 50
    assert int(profile["tracemalloc_peak_bytes"]) > 0
    assert int(profile["approx_bytes_per_node"]) > 0


def test_probe_events_cover_completed_and_excluded_parser_events() -> None:
    parser_events = _events(PARSER_EVENTS)
    parser_completed = {
        event["article_ref"]
        for event in parser_events
        if event.get("event") == "parser_chunking_complete"
    }
    parser_skipped = {
        event["article_ref"]
        for event in parser_events
        if event.get("event") == "parser_chunking_skipped_metadata_only"
    }
    probe_events = _events(EVENTS_LOG)
    added = {event["article_ref"] for event in probe_events if event["event"] == "article_added"}
    excluded = {
        event["article_ref"] for event in probe_events if event["event"] == "metadata_only_excluded"
    }
    assert added == parser_completed
    assert excluded == parser_skipped
    assert len(added) == EXPECTED_COMPLETED
    assert len(excluded) == EXPECTED_SKIPPED


def test_build_request_records_repo_relative_input_artifact_path() -> None:
    request = build_request(
        R024NetworkXProbeConfig(
            corpus_id="r024-218-document-corpus-v1",
            corpus_dir=R218_DIR,
            parser_events_path=PARSER_EVENTS,
            probe_dir=PROBE_DIR,
            graphml_path=GRAPHML,
            summary_path=SUMMARY,
            events_path=EVENTS_LOG,
            summary_schema_version="test-summary.v1",
            repo_root=REPO_ROOT,
        )
    )

    assert request.input_artifacts[0].path == (
        "data/r024-218-document-corpus-v1/parser-chunking/events.jsonl"
    )


def test_probe_events_fail_closed() -> None:
    for event in _events(EVENTS_LOG):
        assert event["network_fetch_attempted"] is False
        assert event["production_import_attempted"] is False
        assert event["graph_import_allowed"] is False
        assert event["ladybugdb_written"] is False


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
