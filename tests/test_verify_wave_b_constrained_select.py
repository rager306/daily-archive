"""Operator tests for constrained select (header/oracle/llm; no live LLM)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_constrained_select.py"
STAMP = ROOT / "artifacts" / "wave-b" / "human_go.json"


def test_no_stamp_blocked_json() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["dspy_optimizer_enabled"] is False
    assert report["llm_used"] is False
    assert report["operator_status"] == "blocked_gate"
    assert report["case_count"] == 0


def test_header_mode_json_with_stamp() -> None:
    if not STAMP.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--stamp", str(STAMP), "--mode", "header", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["dspy_optimizer_enabled"] is False
    assert report["llm_used"] is False
    assert report["select_mode"] == "header"
    assert report["operator_status"] == "header_priority_select"
    assert report["scored_case_count"] >= 1
    assert report["metrics"] is not None
    assert "entity_f1" in report["metrics"]


def test_oracle_mode_json_with_stamp() -> None:
    if not STAMP.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--stamp", str(STAMP), "--mode", "oracle", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["llm_used"] is False
    assert report["select_mode"] == "oracle"
    assert report["operator_status"] == "lexical_oracle_diagnostic"
    assert report["metrics"]["entity_f1"] == 1.0


def test_llm_mode_without_live_uses_header_fallback_or_requires_flag() -> None:
    """--mode llm without --live-llm must not call network; fail-closed or header fallback."""
    if not STAMP.is_file():
        return
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp",
            str(STAMP),
            "--mode",
            "llm",
            "--json",
            "--case-limit",
            "1",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["dspy_optimizer_enabled"] is False
    # without --live-llm: dry path, not live call
    assert report.get("llm_used") is False or report.get("operator_status") in {
        "llm_constrained_select_dry",
        "llm_requires_live_flag",
        "blocked_gate",
    }
