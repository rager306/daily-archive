from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "sync_codebase_memory_governance.py"

spec = importlib.util.spec_from_file_location("sync_codebase_memory_governance", MODULE_PATH)
assert spec is not None
sync = importlib.util.module_from_spec(spec)
sys.modules["sync_codebase_memory_governance"] = sync
assert spec.loader is not None
spec.loader.exec_module(sync)


def test_current_digest_contains_hybrid_governance_markers() -> None:
    digest = sync.generate_digest()

    assert "GSD remains canonical" in digest
    assert "GitNexus remains mandatory" in digest
    assert "codebase-memory MCP | Fast semantic ADR/R/D recall mirror | no" in digest
    assert "D075" in digest
    assert "D076" in digest
    assert "R062" in digest
    assert "R063" in digest
    assert "ADR-005 blocks direct extractor" in digest
    assert ".codebase-memory/governance-graph.json" in digest


def test_parse_requirements_extracts_compact_index_fields() -> None:
    text = """
# Requirements

### R999 — Example requirement
- Class: operability
- Status: active
- Description: Long description not copied separately.
- Why it matters: Why text.
- Source: test source
- Primary owning slice: M999/S01
"""

    entries = sync.parse_requirements(text)

    assert entries == [
        sync.RequirementEntry(
            req_id="R999",
            title="Example requirement",
            status="active",
            source="test source",
            owner="M999/S01",
        )
    ]


def test_parse_decisions_extracts_compact_index_fields() -> None:
    text = """
| # | When | Scope | Decision | Choice | Rationale | Revisable? | Made By |
|---|------|-------|----------|--------|-----------|------------|---------|
| D999 | M999 | governance-memory | Store mirror | Generate digest | Rationale should not be rendered from this test. | Yes | agent |
"""

    entries = sync.parse_decisions(text)

    assert entries == [
        sync.DecisionEntry(
            decision_id="D999",
            when="M999",
            scope="governance-memory",
            decision="Store mirror",
            choice="Generate digest",
        )
    ]


def test_render_digest_rejects_secret_shaped_content() -> None:
    with pytest.raises(ValueError, match="secret-shaped"):
        sync.render_digest(
            requirements=[
                sync.RequirementEntry(
                    req_id="R999",
                    title="bad api_key=secret-value",
                    status="active",
                    source="test",
                    owner="test",
                )
            ],
            decisions=[],
            adrs=[],
        )


def test_generate_graph_contains_required_typed_nodes_and_edges() -> None:
    graph = sync.generate_graph()

    assert graph["schema_version"] == "governance-graph/v1"
    assert graph["mirror_only"] is True
    node_ids = {node["id"] for node in graph["nodes"]}
    edge_keys = {(edge["source"], edge["relationship"], edge["target"]) for edge in graph["edges"]}

    assert {"D075", "D076", "R062", "R063", "ADR-005", "M038", "M039"} <= node_ids
    assert ("D076", "extends", "D075") in edge_keys
    assert ("D076", "implements", "R063") in edge_keys
    assert ("R063", "owned_by", "M039") in edge_keys
    assert ("ADR-005", "blocks", "SAFETY-NO-DIRECT-GRAPHDB-WRITES") in edge_keys


def test_render_graph_is_valid_json() -> None:
    rendered = sync.render_graph(sync.generate_graph())

    parsed = json.loads(rendered)
    assert parsed["source_of_truth_warning"].startswith("GSD remains canonical")


def test_validate_graph_rejects_missing_required_edge() -> None:
    graph = sync.generate_graph()
    graph["edges"] = [
        edge
        for edge in graph["edges"]
        if (edge["source"], edge["relationship"], edge["target"]) != ("D076", "extends", "D075")
    ]

    with pytest.raises(ValueError, match="missing required edges"):
        sync.validate_graph(graph)


def test_render_digest_rejects_forbidden_payload_terms() -> None:
    with pytest.raises(ValueError, match="forbidden payload term"):
        sync.render_digest(
            requirements=[],
            decisions=[
                sync.DecisionEntry(
                    decision_id="D999",
                    when="test",
                    scope="test",
                    decision="Persist full_text",
                    choice="never",
                )
            ],
            adrs=[],
        )


def test_check_digest_fails_when_output_is_stale(tmp_path: Path) -> None:
    output = tmp_path / "adr.md"
    graph_output = tmp_path / "governance-graph.json"
    output.write_text("stale\n", encoding="utf-8")
    graph_output.write_text(sync.render_graph(sync.generate_graph()), encoding="utf-8")

    with pytest.raises(SystemExit, match="stale governance mirror artifacts"):
        sync.check_outputs(output, graph_output)


def test_check_outputs_fails_when_graph_is_stale(tmp_path: Path) -> None:
    output = tmp_path / "adr.md"
    graph_output = tmp_path / "governance-graph.json"
    output.write_text(sync.generate_digest(), encoding="utf-8")
    graph_output.write_text("{}\n", encoding="utf-8")

    with pytest.raises(SystemExit, match="governance-graph.json"):
        sync.check_outputs(output, graph_output)
