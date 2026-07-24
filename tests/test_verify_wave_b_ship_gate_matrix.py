"""Operator tests for Wave B ship-gate matrix (M260)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_ship_gate_matrix.py"


def test_ship_matrix_operator_skip_live_uses_artifacts() -> None:
    """Skip live score still emits matrix from disk compare/header if present."""
    out = ROOT / "artifacts" / "wave-b" / "ship-gate-matrix-test.json"
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--skip-live-score",
            "--json",
            "--output",
            str(out),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["dspy_optimizer_enabled"] is False
    assert "relation_status" in report
    assert "worlds" in report
    assert "ship_path" in report
    # Prefer header path while LLM loses
    assert report["gepa_justified"] is False
