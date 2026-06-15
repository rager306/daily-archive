from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADR_021 = ROOT / "doc" / "adr" / "ADR-021-graphdb-reselection.md"
ADR_020 = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
ADR_INDEX = ROOT / "doc" / "adr" / "ADR-INDEX.md"
CODEBASE_MEMORY_ADR = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH = ROOT / ".codebase-memory" / "governance-graph.json"
BENCHMARK_DATA = ROOT / "artifacts" / "m066-graphdb-reselection" / "benchmark-data.json"
SCORING_MATRIX = ROOT / "artifacts" / "m066-graphdb-reselection" / "scoring-matrix.md"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_adr_021_exists() -> None:
    assert ADR_021.exists()
    text = read(ADR_021)
    assert text.startswith("# ADR-021: GraphDB Re-Selection for M066")
    assert "**Status:** Accepted (binding)" in text
    assert "**Binding:** yes" in text


def test_adr_021_full_m034_template() -> None:
    text = read(ADR_021)
    sections = re.findall(r"^## (\d+)\. ", text, flags=re.MULTILINE)
    assert len(sections) >= 16
    assert sections[:17] == [str(index) for index in range(17)]
    assert "## 15. LLM Reading Notes" in text
    assert "## 16. Amendment Log" in text
    assert text.count("```mermaid") >= 2
    assert "prose and tables below are authoritative" in text


def test_adr_021_explicit_choice() -> None:
    text = read(ADR_021)
    assert "Neo4j" in text
    assert "76/90" in text
    assert "29/30" in text
    assert "production GraphDB" in text


def test_adr_021_supersedes_adr_020() -> None:
    text = read(ADR_021)
    assert "Supersedes | ADR-020" in text
    assert "ADR-020 is superseded by this ADR." in text
    assert "LadybugDB showed **33% concurrent write success**" in text
    assert "199 lost writes out of 300 attempted writes" in text


def test_adr_020_amendment_log_present() -> None:
    text = read(ADR_020)
    assert "## Amendment Log" in text
    assert "SUPERSEDED by ADR-021" in text
    assert "Neo4j chosen as production GraphDB (76/90 score)" in text
    assert "LadybugDB (62/90)" in text
    assert "concurrent write semantics (LadybugDB 33% success under load)" in text
    assert text.index("## Amendment Log") < text.index("## 13. LLM Reading Notes")


def test_adr_index_updated() -> None:
    text = read(ADR_INDEX)
    assert "Project-level ADR count: 21" in text
    assert "| ADR-020 | Superseded by ADR-021 |" in text
    assert "`doc/adr/ADR-020-graphdb-selection.md`" in text
    assert "| ADR-021 | Accepted (binding) | M066 GraphDB Re-Selection (Neo4j primary) | `doc/adr/ADR-021-graphdb-reselection.md` |" in text


def test_codebase_memory_synced() -> None:
    adr_mirror = read(CODEBASE_MEMORY_ADR)
    graph = json.loads(read(CODEBASE_MEMORY_GRAPH))

    assert "ADR-021" in adr_mirror
    assert "doc/adr/ADR-021-graphdb-reselection.md" in adr_mirror
    adr_node = next(node for node in graph["nodes"] if node["id"] == "ADR-021")
    assert adr_node["title"] == "GraphDB Re-Selection for M066"
    assert adr_node["status"] == "Accepted (binding)"
    assert adr_node["type"] == "ADR"


def test_m050_m066_s01_regression() -> None:
    data = json.loads(read(BENCHMARK_DATA))
    matrix = read(SCORING_MATRIX)
    by_name = {candidate["name"]: candidate for candidate in data["candidates"]}

    assert data["winner"] == "Neo4j"
    assert data["top_3"] == ["Neo4j", "FalkorDB", "Apache AGE"]
    assert {key: value for key, value in data["safety_defaults"].items()} == {
        "graph_writes_enabled_by_default": False,
        "network_enabled_by_default": False,
        "production_import_enabled_by_default": False,
        "real_db_connection_enabled_by_default": False,
        "vendor_source_mutation_enabled_by_default": False,
    }
    assert by_name["Neo4j"]["total_score"] == 76
    assert by_name["FalkorDB"]["total_score"] == 68
    assert by_name["Apache AGE"]["total_score"] == 64
    assert by_name["LadybugDB"]["total_score"] == 62
    assert by_name["HelixDB"]["total_score"] == 54
    assert by_name["LadybugDB"]["concurrent_write_metrics"]["lost_writes"] == 199
    assert "Production graph import is not authorized." in matrix
    assert "Network access, production import, graph writes, vendor-source mutation, and real DB connections are disabled." in matrix
