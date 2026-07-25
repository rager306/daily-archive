"""M255 S02: operator hybrid statistical extraction script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_hybrid_statistical_extraction.py"


def test_no_stamp_blocked_json() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--sample-limit", "2", "--json"],
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
    assert report["fleet_status"] == "blocked_gate"
    assert report["wave_b_gate_open"] is False


def test_live_stamp_if_present() -> None:
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--sample-limit", "3", "--json"],
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
    if stamp.is_file() and report.get("human_go") is True:
        assert report["fleet_status"] == "sampled"
        assert report["wave_b_gate_open"] is True
        assert report["paper_count"] <= 3


def test_summary_line_no_stamp() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--sample-limit", "1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-hybrid-statistical-extraction" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "llm: false" in proc.stdout
