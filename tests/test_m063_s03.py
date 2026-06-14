from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = ROOT / "artifacts" / "m063-graphdb" / "REPORT.md"
SUMMARY_PATH = ROOT / ".gsd" / "milestones" / "M065-u29n4f" / "M065-u29n4f-SUMMARY.md"
VALIDATION_PATH = ROOT / ".gsd" / "milestones" / "M065-u29n4f" / "M065-u29n4f-VALIDATION.md"
ADR_020_PATH = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
SCORING_MATRIX_PATH = ROOT / "artifacts" / "m063-graphdb" / "scoring-matrix.md"
LADYBUGDB_REPORT_PATH = ROOT / "artifacts" / "m063-graphdb" / "candidates" / "ladybugdb-report.md"
CODEBASE_MEMORY_ADR_PATH = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH_PATH = ROOT / ".codebase-memory" / "governance-graph.json"
M045_TRAJECTORY_PATH = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.md"
M044_GUARDRAIL_PATH = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"

REGRESSION_TEST_FILES = [
    "tests/test_m050_e2e_pipeline.py",
    "tests/test_m063_s01.py",
    "tests/test_m063_s02.py",
]


def test_report_md_exists() -> None:
    assert REPORT_PATH.exists()
    text = REPORT_PATH.read_text(encoding="utf-8")
    assert text.startswith("# M063 GraphDB selection")
    assert "LadybugDB" in text
    assert "39/45" in text


def test_report_8_sections() -> None:
    text = REPORT_PATH.read_text(encoding="utf-8")
    sections = re.findall(r"^## ([0-7])\. ", text, flags=re.MULTILINE)
    assert sections == [str(index) for index in range(8)]
    assert re.search(r"^## 8\. ", text, flags=re.MULTILINE) is None


def test_m063_closeout_artifacts() -> None:
    assert SUMMARY_PATH.exists()
    assert VALIDATION_PATH.exists()
    summary = SUMMARY_PATH.read_text(encoding="utf-8")
    validation = VALIDATION_PATH.read_text(encoding="utf-8")
    assert "status: complete" in summary
    assert "verdict: pass" in validation
    assert "M045 trajectory closeout remains on_track" in validation
    assert "M044 guardrail remains ok" in validation
    assert "Graph import is not authorized" in validation
    assert "Graph writes is disabled" in validation


def test_ladybugdb_choice_documented() -> None:
    combined = "\n".join(
        path.read_text(encoding="utf-8")
        for path in [REPORT_PATH, SUMMARY_PATH, VALIDATION_PATH, SCORING_MATRIX_PATH, LADYBUGDB_REPORT_PATH]
    )
    assert "LadybugDB" in combined
    assert "primary production GraphDB" in combined
    assert "39/45" in combined
    assert "FalkorDB" in combined and "35/45" in combined
    assert "Neo4j" in combined and "34/45" in combined


def test_adr_020_referenced() -> None:
    assert ADR_020_PATH.exists()
    report = REPORT_PATH.read_text(encoding="utf-8")
    summary = SUMMARY_PATH.read_text(encoding="utf-8")
    validation = VALIDATION_PATH.read_text(encoding="utf-8")
    adr_text = ADR_020_PATH.read_text(encoding="utf-8")
    assert "ADR-020" in report
    assert "ADR-020" in summary
    assert "ADR-020" in validation
    assert "LadybugDB" in adr_text
    assert "binding" in adr_text.lower()


def test_code_memory_synced() -> None:
    text = CODEBASE_MEMORY_ADR_PATH.read_text(encoding="utf-8")
    adr_rows = re.findall(r"^\| ADR-\d+", text, flags=re.MULTILINE)
    assert len(adr_rows) >= 20
    assert "ADR-020" in text
    assert "LadybugDB" in text
    assert "doc/adr/ADR-020-graphdb-selection.md" in text

    graph = json.loads(CODEBASE_MEMORY_GRAPH_PATH.read_text(encoding="utf-8"))
    node_ids = {node["id"] for node in graph["nodes"]}
    assert graph["mirror_only"] is True
    assert "ADR-020" in node_ids


def test_m045_on_track_and_m044_ok() -> None:
    m045_text = M045_TRAJECTORY_PATH.read_text(encoding="utf-8")
    m044_text = M044_GUARDRAIL_PATH.read_text(encoding="utf-8")
    assert "Verdict: `on_track`" in m045_text
    assert "No graph import is authorized" in m044_text
    assert "Graph writes: disabled" in m044_text


def test_m050_m063_s01_s02_regression() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "pytest", *REGRESSION_TEST_FILES, "-q"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
        timeout=300,
    )
    assert result.returncode == 0, result.stdout + result.stderr
