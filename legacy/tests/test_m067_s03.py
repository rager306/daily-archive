from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

from scripts import m066_graphdb_full_benchmark as benchmark

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_DIR = ROOT / "artifacts" / "m066-graphdb-reselection"
REPORT = ARTIFACT_DIR / "REPORT.md"
SUMMARY = ROOT / ".gsd" / "milestones" / "M067-oqsavh" / "M067-oqsavh-SUMMARY.md"
VALIDATION = ROOT / ".gsd" / "milestones" / "M067-oqsavh" / "M067-oqsavh-VALIDATION.md"
ADR_022 = ROOT / "doc" / "adr" / "ADR-022-graphdb-reselection-self-hosted.md"
ADR_021 = ROOT / "doc" / "adr" / "ADR-021-graphdb-reselection.md"
ADR_020 = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
SCORING_MATRIX = ARTIFACT_DIR / "scoring-matrix.md"
DISTRIBUTION_MODEL = ARTIFACT_DIR / "distribution-model.md"
FALKOR_REPORT = ARTIFACT_DIR / "candidates" / "falkordb-report.md"
CODEBASE_MEMORY_ADR = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH = ROOT / ".codebase-memory" / "governance-graph.json"
SYNC_SCRIPT = ROOT / "scripts" / "sync_codebase_memory_governance.py"
BENCHMARK_SCRIPT = ROOT / "scripts" / "m066_graphdb_full_benchmark.py"
M045_REPORT = ROOT / "artifacts" / "m045-project-trajectory" / "current" / "trajectory-report.json"
M044_REPORT = ROOT / "artifacts" / "m044-grobid-architecture-guardrail" / "final-report.md"
M050_E2E = ROOT / "tests" / "test_m050_e2e_pipeline.py"
M067_S01 = ROOT / "tests" / "test_m067_s01.py"
M067_S02 = ROOT / "tests" / "test_m067_s02.py"

def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def canonical_adr_files() -> list[Path]:
    return [
        path
        for path in (ROOT / "doc" / "adr").rglob("ADR-*.md")
        if "TEMPLATE" not in path.name and "INDEX" not in path.name
    ]


def test_report_md_exists() -> None:
    assert REPORT.exists()
    text = read(REPORT)
    assert text.startswith("# M067: повторный выбор GraphDB")
    assert "FalkorDB" in text
    assert "70/90" in text
    assert "SSPLv1" in text


def test_report_8_sections() -> None:
    text = read(REPORT)
    sections = re.findall(r"^## (\d)\. ", text, flags=re.MULTILINE)
    assert sections == [str(index) for index in range(8)]
    assert "## 8." not in text
    assert "ADR-022" in text
    assert "ADR-021" in text
    assert "ADR-020" in text


def test_m067_closeout_artifacts() -> None:
    assert SUMMARY.exists()
    assert VALIDATION.exists()
    summary = read(SUMMARY)
    validation = read(VALIDATION)

    assert "id: M067-oqsavh" in summary
    assert "status: complete" in summary
    assert "M067 corrected the FalkorDB license analysis" in summary
    assert "verdict: pass" in validation
    assert "M045 stays on_track" in summary
    assert "M044 stays ok" in summary
    loopback_address = ".".join(["127", "0", "0", "1"])
    assert loopback_address not in validation


def test_falkordb_choice_documented() -> None:
    combined = "\n".join([read(REPORT), read(SUMMARY), read(VALIDATION)])

    assert "FalkorDB" in combined
    assert "70/90" in combined
    assert "SSPLv1" in combined
    assert "Apache AGE" in combined and "64/90" in combined
    assert "LadybugDB" in combined and "62/90" in combined
    assert "Neo4j" in combined and "76/90" in combined
    assert "22/30" in combined


def test_adr_022_referenced() -> None:
    report = read(REPORT)
    summary = read(SUMMARY)
    validation = read(VALIDATION)
    adr = read(ADR_022)

    assert ADR_022.exists()
    assert "# ADR-022: GraphDB Re-Selection Self-Hosted" in adr
    assert "Accepted (binding)" in adr
    assert "ADR-022" in report
    assert "ADR-022" in summary
    assert "ADR-022" in validation
    assert "binding" in summary


def test_adr_021_supersede_documented() -> None:
    combined = "\n".join(
        [read(REPORT), read(SUMMARY), read(VALIDATION), read(ADR_021), read(ADR_020)]
    )

    assert "ADR-021" in combined
    assert "ADR-020" in combined
    assert "superseded by ADR-022" in combined
    assert "SUPERSEDED by ADR-022" in read(ADR_021)
    assert "SUPERSEDED again by ADR-022" in read(ADR_020)


def test_code_memory_synced() -> None:
    result = subprocess.run(
        [sys.executable, str(SYNC_SCRIPT)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr

    adr_text = read(CODEBASE_MEMORY_ADR)
    graph = json.loads(read(CODEBASE_MEMORY_GRAPH))
    canonical_ids = {path.stem.split("-")[1] for path in canonical_adr_files()}

    assert len(canonical_ids) >= 22
    assert (
        "| ADR-022 | Accepted (binding) | `doc/adr/ADR-022-graphdb-reselection-self-hosted.md` | GraphDB Re-Selection Self-Hosted |"
        in adr_text
    )
    for adr_number in sorted(canonical_ids):
        assert f"ADR-{adr_number}" in adr_text

    graph_ids = {node["id"] for node in graph["nodes"]}
    assert "ADR-022" in graph_ids
    assert all(f"ADR-{adr_number}" in graph_ids for adr_number in canonical_ids)


def test_m050_m067_s01_s02_regression() -> None:
    matrix = read(SCORING_MATRIX)
    distribution = read(DISTRIBUTION_MODEL)
    falkor = read(FALKOR_REPORT)

    assert M050_E2E.exists()
    assert M067_S01.exists()
    assert M067_S02.exists()
    assert "on_track" in read(M045_REPORT)
    assert "No graph import is authorized" in read(M044_REPORT)
    assert "| **Total score** | **70/90**" in matrix
    assert "FalkorDB 70/90 > Apache AGE 64/90 > LadybugDB 62/90" in matrix
    assert "daily-archive is a self-hosted research project" in distribution
    assert "future daily-archive distribution model is uncertain" in distribution
    assert "**Total score:** 70/90" in falkor


def test_safety_defaults_remain_false() -> None:
    assert benchmark.SAFETY_DEFAULTS == {
        "network_enabled_by_default": False,
        "production_import_enabled_by_default": False,
        "graph_writes_enabled_by_default": False,
        "vendor_source_mutation_enabled_by_default": False,
        "real_db_connection_enabled_by_default": False,
    }
    assert all(value is False for value in benchmark.SAFETY_DEFAULTS.values())
