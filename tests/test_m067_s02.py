from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADR_022 = ROOT / "doc" / "adr" / "ADR-022-graphdb-reselection-self-hosted.md"
ADR_021 = ROOT / "doc" / "adr" / "ADR-021-graphdb-reselection.md"
ADR_020 = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
ADR_INDEX = ROOT / "doc" / "adr" / "ADR-INDEX.md"
CODEBASE_MEMORY_ADR = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH = ROOT / ".codebase-memory" / "governance-graph.json"
SCORING_MATRIX = ROOT / "artifacts" / "m066-graphdb-reselection" / "scoring-matrix.md"
DISTRIBUTION_MODEL = ROOT / "artifacts" / "m066-graphdb-reselection" / "distribution-model.md"
FALKOR_REPORT = ROOT / "artifacts" / "m066-graphdb-reselection" / "candidates" / "falkordb-report.md"
M045_REPORT = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.json"
M044_REPORT = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"
M050_E2E = ROOT / "tests" / "test_m050_e2e_pipeline.py"
M067_S01 = ROOT / "tests" / "test_m067_s01.py"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_adr_022_exists() -> None:
    text = read(ADR_022)
    assert text.startswith("# ADR-022: GraphDB Re-Selection Self-Hosted")
    assert "**Status:** Accepted (binding)" in text
    assert "**Binding:** yes" in text


def test_adr_022_full_m034_template() -> None:
    text = read(ADR_022)
    sections = re.findall(r"^## (\d+)\. ", text, flags=re.MULTILINE)
    assert len(sections) >= 16
    assert sections[:17] == [str(index) for index in range(17)]
    assert "## 15. LLM Reading Notes" in text
    assert "## 16. Amendment Log" in text
    assert text.count("```mermaid") >= 2
    assert "prose and tables below are authoritative" in text


def test_adr_022_explicit_choice() -> None:
    text = read(ADR_022)
    assert "FalkorDB" in text
    assert "70/90" in text
    assert "self-hosted production GraphDB" in text
    assert "SSPLv1" in text
    assert "AGPLv3" in text


def test_adr_022_supersedes_adr_021_and_adr_020() -> None:
    text = read(ADR_022)
    assert "ADR-022 supersedes both prior GraphDB selection ADRs" in text
    assert "| ADR-021 | Neo4j, **76/90** | ADR-022 |" in text
    assert "| ADR-020 | LadybugDB, originally **39/45** and later **62/90** | ADR-022 |" in text
    assert "ADR021 --> ADR022" in text


def test_adr_021_amendment_log_present() -> None:
    text = read(ADR_021)
    assert "## 16. Amendment Log" in text
    assert "SUPERSEDED by ADR-022" in text
    assert "FalkorDB chosen for self-hosted daily-archive (70/90 score) instead of Neo4j (76/90)" in text
    assert "FalkorDB = SSPLv1 (NOT AGPLv3, NOT RSAL 2.0)" in text
    assert "SaaS triggers Section 13 OR commercial license" in text


def test_adr_020_amendment_log_2nd_entry_present() -> None:
    text = read(ADR_020)
    assert "## Amendment Log" in text
    assert text.count("SUPERSEDED") >= 2
    assert "SUPERSEDED again by ADR-022" in text
    assert "Original M063 LadybugDB choice 39/45 superseded by ADR-021" in text
    assert "FalkorDB 70/90 for self-hosted" in text
    assert text.index("SUPERSEDED by ADR-021") < text.index("SUPERSEDED again by ADR-022")


def test_adr_index_updated() -> None:
    text = read(ADR_INDEX)
    assert "Project-level ADR count: 22" in text
    assert "| ADR-020 | Superseded by ADR-022 | M063 GraphDB Selection (LadybugDB primary) | `doc/adr/ADR-020-graphdb-selection.md` |" in text
    assert "| ADR-021 | Superseded by ADR-022 | M066 GraphDB Re-Selection (Neo4j primary) | `doc/adr/ADR-021-graphdb-reselection.md` |" in text
    assert "| ADR-022 | Accepted (binding) | M067 GraphDB Re-Selection Self-Hosted (FalkorDB primary) | `doc/adr/ADR-022-graphdb-reselection-self-hosted.md` |" in text


def test_codebase_memory_synced() -> None:
    adr_text = read(CODEBASE_MEMORY_ADR)
    graph = json.loads(read(CODEBASE_MEMORY_GRAPH))
    assert "| ADR-022 | Accepted (binding) | `doc/adr/ADR-022-graphdb-reselection-self-hosted.md` | GraphDB Re-Selection Self-Hosted |" in adr_text
    adr_node = next(node for node in graph["nodes"] if node["id"] == "ADR-022")
    assert adr_node["title"] == "GraphDB Re-Selection Self-Hosted"
    assert adr_node["status"] == "Accepted (binding)"
    assert adr_node["type"] == "ADR"


def test_m050_m067_s01_regression() -> None:
    matrix = read(SCORING_MATRIX)
    distribution = read(DISTRIBUTION_MODEL)
    falkor = read(FALKOR_REPORT)

    assert M050_E2E.exists()
    assert M067_S01.exists()
    assert "on_track" in read(M045_REPORT)
    assert "No graph import is authorized" in read(M044_REPORT)
    assert "| **Total score** | **70/90**" in matrix
    assert "FalkorDB 70/90 > Apache AGE 64/90 > LadybugDB 62/90" in matrix
    assert "daily-archive is a self-hosted research project" in distribution
    assert "future daily-archive distribution model is uncertain" in distribution
    assert "**Total score:** 70/90" in falkor
