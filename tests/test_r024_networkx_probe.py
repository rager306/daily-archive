"""Tests for R024 S04: bounded NetworkX probe (10 articles, fail-closed)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
PROBE_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "networkx-probe"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
EVENTS_LOG = PROBE_DIR / "events.jsonl"
SELECTION = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "selection.json"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def test_probe_dir_exists() -> None:
    assert PROBE_DIR.exists(), "networkx-probe dir missing"


def test_graphml_exists() -> None:
    assert GRAPHML.exists(), "probe.graphml missing"


def test_summary_exists() -> None:
    assert SUMMARY.exists(), "summary.json missing"


def test_summary_fail_closed_invariants() -> None:
    s = _load(SUMMARY)
    fc = s.get("fail_closed_invariants")
    assert isinstance(fc, dict)
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
        assert fc.get(key) is False, f"{key} must be False"


def test_summary_no_db_connection() -> None:
    s = _load(SUMMARY)
    impl = s.get("implementation")
    assert isinstance(impl, dict)
    assert impl.get("library") == "networkx"
    assert impl.get("no_db_connection") is True
    assert impl.get("no_network_io") is True
    assert impl.get("in_memory_only") is True


def test_summary_10_articles() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("corpus_size", 0))) == 10


def test_summary_nodes_articles_chunks() -> None:
    s = _load(SUMMARY)
    nt = s.get("node_types")
    assert isinstance(nt, dict)
    assert int(str(nt.get("article", 0))) == 10
    assert int(str(nt.get("chunk", 0))) == 20
    assert int(str(nt.get("corpus", 0))) == 1


def test_summary_edges() -> None:
    s = _load(SUMMARY)
    et = s.get("edge_types")
    assert isinstance(et, dict)
    assert int(str(et.get("corpus_contains_article", 0))) == 10
    assert int(str(et.get("article_contains_chunk", 0))) == 20


def test_summary_n_nodes_positive() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("n_nodes", 0))) >= 21  # 1 corpus + 10 articles + 20 chunks


def test_graphml_loadable() -> None:
    """graphml file is parseable by networkx."""
    nx = pytest.importorskip("networkx")
    g = nx.read_graphml(str(GRAPHML))
    assert g.number_of_nodes() == 31
    assert g.number_of_edges() == 30


def test_events_log_fail_closed() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    assert len(events) == 10
    for e in events:
        assert e["network_fetch_attempted"] is False
        assert e["production_import_attempted"] is False
        assert e["graph_import_allowed"] is False
        assert e["ladybugdb_written"] is False


def test_selection_matches_events() -> None:
    sel_data = json.loads(SELECTION.read_text())
    articles = cast(list[dict[str, object]], sel_data["articles"])
    sel_refs = {str(a["article_ref"]) for a in articles}
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    event_refs = {str(e["article_ref"]) for e in events}
    assert sel_refs == event_refs


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
