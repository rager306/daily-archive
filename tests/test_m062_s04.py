from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = ROOT / "artifacts" / "m062-fd-contract" / "REPORT.md"
ADR_PATH = ROOT / "doc" / "adr" / "ADR-019-fd-embedding-service-contract.md"
SUMMARY_PATH = ROOT / ".gsd" / "milestones" / "M065-vq0do4" / "M065-vq0do4-SUMMARY.md"
VALIDATION_PATH = ROOT / ".gsd" / "milestones" / "M065-vq0do4" / "M065-vq0do4-VALIDATION.md"
CODEBASE_MEMORY_ADR_PATH = ROOT / ".codebase-memory" / "adr.md"

REGRESSION_TEST_FILES = [
    "tests/test_m050_article_artifact_reducer.py",
    "tests/test_m050_article_artifact_worker.py",
    "tests/test_m050_e2e_pipeline.py",
    "tests/test_m062_s01.py",
    "tests/test_m062_s02.py",
    "tests/test_m062_s03.py",
]


def test_report_md_exists() -> None:
    assert REPORT_PATH.exists()
    text = REPORT_PATH.read_text(encoding="utf-8")
    assert text.startswith("# M062 fd production hardening")
    assert "M062" in text
    assert "127.0.0.1" in text
    assert "localhost" not in text.lower()


def test_report_8_sections() -> None:
    text = REPORT_PATH.read_text(encoding="utf-8")
    sections = re.findall(r"^## ([0-7])\. ", text, flags=re.MULTILINE)
    assert sections == [str(index) for index in range(8)]
    assert "## 8." not in text


def test_adr_019_amended_with_env_section() -> None:
    text = ADR_PATH.read_text(encoding="utf-8")
    assert "## 4.5 Configuration: env-driven" in text
    assert "FD_EMBEDDINGS_ENDPOINT | http://127.0.0.1:8000/v1/embeddings" in text
    assert "FD_CIRCUIT_OPEN_SECONDS" in text
    assert "hardcoded values" in text


def test_adr_019_amendment_log_present() -> None:
    text = ADR_PATH.read_text(encoding="utf-8")
    assert "## Amendment Log" in text
    assert "2026-06-14 | user feedback (executor-01)" in text
    assert "all FD service config should be env-driven" in text
    assert text.index("## Amendment Log") < text.index("## 14. LLM Reading Notes")


def test_m062_closeout_artifacts() -> None:
    assert SUMMARY_PATH.exists()
    assert VALIDATION_PATH.exists()
    summary = SUMMARY_PATH.read_text(encoding="utf-8")
    validation = VALIDATION_PATH.read_text(encoding="utf-8")
    assert "status: complete" in summary
    assert "verdict: pass" in validation
    assert "M045 trajectory closeout remains on_track" in validation
    assert "M044 guardrail remains ok" in validation


def test_code_memory_synced() -> None:
    text = CODEBASE_MEMORY_ADR_PATH.read_text(encoding="utf-8")
    adr_rows = re.findall(r"^\| ADR-\d+", text, flags=re.MULTILINE)
    assert len(adr_rows) >= 15
    assert "ADR-019" in text
    assert "doc/adr/ADR-019-fd-embedding-service-contract.md" in text


def test_m050_m062_s01_s02_s03_regression() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "pytest", *REGRESSION_TEST_FILES, "-q"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
        timeout=180,
    )
    assert result.returncode == 0, result.stdout + result.stderr
