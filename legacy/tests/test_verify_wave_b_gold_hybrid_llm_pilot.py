"""Operator tests for Wave B gold-hybrid LLM pilot (no live LLM required)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_gold_hybrid_llm_pilot.py"


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
    assert report["llm_used"] is False
    assert report["dspy_optimizer_enabled"] is False
    assert report["operator_status"] == "blocked_gate"
    assert report["wave_b_gate_open"] is False
    assert report["scored_case_count"] == 0


def test_dry_run_floor_only_with_stamp() -> None:
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
    if not stamp.is_file():
        return
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp",
            str(stamp),
            "--dry-run-floor-only",
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["llm_used"] is False
    assert report["operator_status"] == "floor_only"
    assert report["floor_metrics"] is not None
    assert report["scored_case_count"] >= 1
