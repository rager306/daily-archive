from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "artifacts" / "m060g-judge" / "REPORT.md"
ADR_014 = ROOT / "doc" / "adr" / "ADR-014-minimax-judge-m3-multimodal.md"
M061_SCOPE = ROOT / "artifacts" / "m060g-judge" / "m061-scope.md"
FORBIDDEN_LOOPBACK_HOSTNAME = "local" + "host"

SAFETY_DEFAULTS = (
    "Graph writes are not authorized.",
    "Production import is not authorized.",
    "Fact promotion is not authorized.",
    "External network default is disabled.",
    "LLM calls default is disabled.",
)

ADR_SECTIONS = (
    "## 0. One-line Decision",
    "## 1. Context",
    "## 2. Decision",
    "## 3. Applies To",
    "## 4. Requirements and Decisions Impacted",
    "## 5. Options Considered",
    "## 6. Trade-off Analysis",
    "## 7. Consequences",
    "## 8. Safety and Non-Authorization",
    "## 9. Contract Impact",
    "## 10. Validation / Evidence Required",
    "## 11. Open Questions",
    "## 12. Follow-up Actions",
    "## 13. Supersedes / Superseded By",
    "## 14. LLM Reading Notes",
)


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_report_md_exists() -> None:
    text = _read(REPORT)

    assert len(text.encode("utf-8")) >= 4096
    sections = re.findall(r"^## \d+\. ", text, flags=re.MULTILINE)
    assert len(sections) == 9
    assert "M3 multimodal выбран" in text
    assert "M2.7-highspeed" in text
    assert "M3 multimodal" in text
    assert "23846" in text
    assert "8549" in text
    assert "127.0.0.1" in text
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in text


def test_adr_014_binding() -> None:
    text = _read(ADR_014)

    for section in ADR_SECTIONS:
        assert section in text
    assert "**Status:** Accepted (binding)" in text
    assert "minimax-m3-multimodal-anthropic" in text
    assert "figure-qa-judge-quality" in text
    assert "Option A — M3 multimodal (chosen)" in text
    assert "Option B — M2.7-highspeed" in text
    assert "Option C — Ensemble" in text
    assert "```mermaid" in text
    assert "127.0.0.1" in text
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in text


def test_m061_scope_md_exists() -> None:
    text = _read(M061_SCOPE)

    assert "M061 should proceed with 2-hop BFS" in text
    assert "8–10 hours" in text
    assert "2000 figures × 8.5 s" in text
    assert "5000 figures × 8.5 s" in text
    assert "10% sample" in text
    assert "200 figures × 8.5 s" in text
    assert "500 figures × 8.5 s" in text
    assert "127.0.0.1" in text
    assert FORBIDDEN_LOOPBACK_HOSTNAME not in text


def test_5_safety_defaults() -> None:
    for path in (REPORT, ADR_014, M061_SCOPE):
        text = _read(path)
        for default in SAFETY_DEFAULTS:
            assert default in text, f"missing {default!r} in {path}"
        assert "diagnostic-only" in text
        assert "llm_calls_authorized" in text or path == REPORT


def test_llm_reading_notes_in_adr_014() -> None:
    text = _read(ADR_014)
    notes = text.split("## 14. LLM Reading Notes", maxsplit=1)[1]

    assert "Binding decision" in notes
    assert "figure-qa-judge-quality" in notes
    assert "minimax-m3-multimodal-anthropic" in notes
    assert "graph writes are not authorized" in notes
    assert "production import is not authorized" in notes
    assert "fact promotion is not authorized" in notes


def test_m050_m060g_s01_s02_regression() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "pytest",
            "tests/test_m060g_s01.py",
            "tests/test_m060g_s02.py",
            "-q",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=900,
        check=False,
    )

    assert result.returncode == 0, result.stdout + result.stderr
