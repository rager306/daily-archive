from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_DIR = ROOT / "artifacts" / "m066-graphdb-reselection"
REPORT = ARTIFACT_DIR / "REPORT.md"
SUMMARY = ROOT / ".gsd" / "milestones" / "M066-7fbv31" / "M066-7fbv31-SUMMARY.md"
VALIDATION = ROOT / ".gsd" / "milestones" / "M066-7fbv31" / "M066-7fbv31-VALIDATION.md"
ADR_021 = ROOT / "doc" / "adr" / "ADR-021-graphdb-reselection.md"
ADR_020 = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
CODEBASE_MEMORY_ADR = ROOT / ".codebase-memory" / "adr.md"
CODEBASE_MEMORY_GRAPH = ROOT / ".codebase-memory" / "governance-graph.json"
SYNC_SCRIPT = ROOT / "scripts" / "sync_codebase_memory_governance.py"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_report_md_exists() -> None:
    assert REPORT.exists()
    text = read(REPORT)
    assert text.startswith("# M066: повторный выбор GraphDB")
    assert "Neo4j" in text
    assert "76/90" in text


def test_report_8_sections() -> None:
    text = read(REPORT)
    sections = re.findall(r"^## (\d)\. ", text, flags=re.MULTILINE)
    assert sections == [str(index) for index in range(8)]
    assert "## 8." not in text
    assert "ADR-020" in text
    assert "ADR-021" in text


def test_m066_closeout_artifacts() -> None:
    assert SUMMARY.exists()
    assert VALIDATION.exists()
    summary = read(SUMMARY)
    validation = read(VALIDATION)

    assert "id: M066-7fbv31" in summary
    assert "status: complete" in summary
    assert "M066 re-evaluated GraphDB" in summary
    assert "verdict: pass" in validation
    assert "M045 on_track" in validation
    assert "M044 ok" in validation
    loopback_address = ".".join(["127", "0", "0", "1"])
    assert loopback_address not in validation


def test_neo4j_choice_documented() -> None:
    report = read(REPORT)
    summary = read(SUMMARY)
    validation = read(VALIDATION)
    combined = "\n".join([report, summary, validation])

    assert "Neo4j" in combined
    assert "76/90" in combined
    assert "FalkorDB" in combined and "68/90" in combined
    assert "Apache AGE" in combined and "64/90" in combined
    assert "LadybugDB" in combined and "62/90" in combined
    assert "29/30" in combined


def test_adr_021_referenced() -> None:
    report = read(REPORT)
    summary = read(SUMMARY)
    validation = read(VALIDATION)
    adr = read(ADR_021)

    assert ADR_021.exists()
    assert "# ADR-021: GraphDB Re-Selection for M066" in adr
    assert "ADR-021" in report
    assert "ADR-021" in summary
    assert "ADR-021" in validation
    assert "Accepted (binding)" in adr


def test_adr_020_supersede_documented() -> None:
    report = read(REPORT)
    summary = read(SUMMARY)
    validation = read(VALIDATION)
    adr_021 = read(ADR_021)
    adr_020 = read(ADR_020)

    assert "ADR-020" in report
    assert "ADR-020" in summary
    assert "ADR-020" in validation
    assert "| Supersedes | ADR-020 |" in adr_021
    assert "ADR-020 is superseded by this ADR" in adr_021
    assert "SUPERSEDED by ADR-021" in adr_020
    assert "LadybugDB" in adr_020


def test_code_memory_synced() -> None:
    check = subprocess.run(
        [sys.executable, str(SYNC_SCRIPT), "--check"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    assert check.returncode == 0, check.stdout + check.stderr

    mirror = read(CODEBASE_MEMORY_ADR)
    graph = json.loads(read(CODEBASE_MEMORY_GRAPH))
    adr_ids = {match for match in re.findall(r"\| (ADR-\d{3}) \|", mirror)}
    graph_adr_ids = {
        node["id"] for node in graph["nodes"] if str(node.get("id", "")).startswith("ADR-")
    }

    assert len(adr_ids) >= 17
    assert "ADR-020" in adr_ids
    assert "ADR-021" in adr_ids
    assert "ADR-021" in graph_adr_ids


def test_m050_m066_s01_s02_regression() -> None:
    regression = subprocess.run(
        [
            sys.executable,
            "-m",
            "pytest",
            "tests/test_m050_article_artifact_reducer.py",
            "tests/test_m050_article_artifact_worker.py",
            "tests/test_m050_e2e_pipeline.py",
            "tests/test_m066_s01.py",
            "tests/test_m066_s02.py",
            "-q",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
        timeout=180,
    )
    assert regression.returncode == 0, regression.stdout + regression.stderr
