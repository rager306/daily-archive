from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADR_PATH = ROOT / "doc" / "adr" / "ADR-020-graphdb-selection.md"
ADR_INDEX_PATH = ROOT / "doc" / "adr" / "ADR-INDEX.md"
CODEBASE_MEMORY_ADR_PATH = ROOT / ".codebase-memory" / "adr.md"

REGRESSION_TEST_FILES = [
    "tests/test_m050_e2e_pipeline.py",
    "tests/test_m063_s01.py",
]


def _adr_text() -> str:
    return ADR_PATH.read_text(encoding="utf-8")


def test_adr_020_exists() -> None:
    assert ADR_PATH.exists()


def test_adr_020_full_m034_template() -> None:
    text = _adr_text()
    numbered_sections = re.findall(r"^## (\d+)\.", text, flags=re.MULTILINE)
    assert numbered_sections == [str(index) for index in range(15)]
    assert "Mermaid" in text
    assert "LLM Reading Notes" in text
    assert "Amendment Log" in text
    assert "Binding:** yes" in text or "Binding:** Yes" in text or "Binding: yes" in text


def test_adr_020_explicit_choice() -> None:
    text = _adr_text()
    assert "LadybugDB" in text
    assert "primary production GraphDB" in text
    assert "39/45" in text


def test_adr_020_migration_plan() -> None:
    text = _adr_text()
    assert "Migration Plan from NetworkX" in text
    assert "NetworkX" in text
    assert "per-paper atomic" in text
    assert "rollback" in text.lower()


def test_adr_020_acceptance_criteria() -> None:
    text = _adr_text()
    assert "Graph load" in text
    assert "< 10s" in text
    assert "p95" in text
    assert "< 50ms" in text
    assert "< 100ms" in text


def test_adr_020_alternatives_considered() -> None:
    text = _adr_text()
    alternatives = ["FalkorDB", "Neo4j", "HelixDB", "Apache AGE"]
    assert sum(name in text for name in alternatives) >= 3
    assert "35/45" in text
    assert "34/45" in text
    assert "30/45" in text
    assert "28/45" in text


def test_adr_index_updated() -> None:
    text = ADR_INDEX_PATH.read_text(encoding="utf-8")
    assert "Project-level ADR count: 20" in text
    assert "| ADR-020 | Accepted (binding) | M063 GraphDB Selection (LadybugDB primary) | `doc/adr/ADR-020-graphdb-selection.md` | M063 GraphDB Selection (LadybugDB primary). |" in text


def test_codebase_memory_synced() -> None:
    text = CODEBASE_MEMORY_ADR_PATH.read_text(encoding="utf-8")
    assert "ADR-020" in text
    assert "LadybugDB" in text


def test_m050_m063_s01_regression() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "pytest", *REGRESSION_TEST_FILES, "-q"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
        timeout=240,
    )
    assert result.returncode == 0, result.stdout + result.stderr
