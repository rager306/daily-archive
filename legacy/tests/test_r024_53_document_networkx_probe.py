"""Tests for R024 S04: extended NetworkX probe at 53-article scale + memory profile."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

REPO_ROOT = Path("/root/daily-archive")
R053_DIR = REPO_ROOT / "data" / "r024-53-document-corpus-v1"
PROBE_DIR = R053_DIR / "networkx-probe"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
MEMORY_PROFILE = PROBE_DIR / "memory-profile.json"
EVENTS_LOG = PROBE_DIR / "events.jsonl"
SELECTION = R053_DIR / "selection.json"


def _load(path: Path) -> dict[str, object]:
    data = json.loads(path.read_text())
    return dict(data) if isinstance(data, dict) else {}


def _load_articles(path: Path) -> list[dict[str, object]]:
    data = json.loads(path.read_text())
    if isinstance(data, dict) and isinstance(data.get("articles"), list):
        return [dict(item) for item in data["articles"]]
    return []


def test_probe_dir_exists() -> None:
    assert PROBE_DIR.exists()


def test_graphml_exists() -> None:
    assert GRAPHML.exists()


def test_summary_exists() -> None:
    assert SUMMARY.exists()


def test_memory_profile_exists() -> None:
    assert MEMORY_PROFILE.exists()


def test_summary_fail_closed() -> None:
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
        assert fc.get(key) is False


def test_summary_implementation_in_memory_only() -> None:
    s = _load(SUMMARY)
    impl = s.get("implementation")
    assert isinstance(impl, dict)
    assert impl.get("library") == "networkx"
    assert impl.get("no_db_connection") is True
    assert impl.get("in_memory_only") is True


def test_summary_53_articles() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("corpus_size", 0))) == 53


def test_summary_node_types_extended() -> None:
    s = _load(SUMMARY)
    nt = s.get("node_types")
    assert isinstance(nt, dict)
    assert int(str(nt.get("article", 0))) == 53
    assert int(str(nt.get("chunk", 0))) == 112
    assert int(str(nt.get("entity", 0))) == 265
    assert int(str(nt.get("corpus", 0))) == 1


def test_summary_edge_types_extended() -> None:
    s = _load(SUMMARY)
    et = s.get("edge_types")
    assert isinstance(et, dict)
    assert int(str(et.get("corpus_contains_article", 0))) == 53
    assert int(str(et.get("article_contains_chunk", 0))) == 112
    assert int(str(et.get("article_has_entity", 0))) == 265
    assert int(str(et.get("article_cites_article", 0))) > 0


def test_summary_total_nodes() -> None:
    s = _load(SUMMARY)
    assert int(str(s.get("n_nodes", 0))) == 431


def test_summary_total_edges() -> None:
    s = _load(SUMMARY)
    n = int(str(s.get("n_edges", 0)))
    assert n >= 430  # 53+112+265 minimum; +199 citations


def test_graphml_loadable() -> None:
    nx = pytest.importorskip("networkx")
    g = nx.read_graphml(str(GRAPHML))
    assert g.number_of_nodes() == 431
    assert g.number_of_edges() >= 430


def test_memory_profile_has_tracemalloc() -> None:
    mp = _load(MEMORY_PROFILE)
    assert mp.get("method") == "tracemalloc"
    assert int(str(mp.get("tracemalloc_current_bytes", 0))) > 0
    assert int(str(mp.get("tracemalloc_peak_bytes", 0))) > 0
    assert float(str(mp.get("peak_mb", 0))) > 0
    assert float(str(mp.get("current_mb", 0))) > 0


def test_memory_profile_bytes_per_node_reasonable() -> None:
    """At 53 articles, peak memory should be under 50 MB (reasonable for in-memory NetworkX)."""
    mp = _load(MEMORY_PROFILE)
    peak = float(str(mp.get("peak_mb", 0)))
    assert peak < 50, f"peak_mb={peak} too high"


def test_events_log_fail_closed() -> None:
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    assert len(events) == 53
    for e in events:
        assert e["network_fetch_attempted"] is False
        assert e["production_import_attempted"] is False
        assert e["graph_import_allowed"] is False
        assert e["ladybugdb_written"] is False


def test_selection_matches_events() -> None:
    sel_articles = _load_articles(SELECTION)
    sel_refs = {str(a["article_ref"]) for a in sel_articles}
    events = [
        dict(json.loads(line)) for line in EVENTS_LOG.read_text().splitlines() if line.strip()
    ]
    event_refs = {str(e["article_ref"]) for e in events}
    assert sel_refs == event_refs


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
    _ = cast  # keep for tooling
