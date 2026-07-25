"""M256 S03: operator gold-linked hybrid lexical metrics."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_gold_hybrid_metrics.py"


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


def test_live_stamp_if_present() -> None:
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
    fixtures = (
        ROOT
        / "artifacts"
        / "m072-reviewed-extraction-benchmark"
        / "fixtures"
        / "train-gold.jsonl"
    )
    if not fixtures.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["llm_used"] is False
    if stamp.is_file() and report.get("human_go") is True:
        assert report["operator_status"] == "sampled"
        assert report["joined_count"] >= 1
        assert report["scored_case_count"] >= 1
        assert "entity_f1" in report["metrics"]


def test_summary_line_no_stamp() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-gold-hybrid-metrics" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "lexical_floor_baseline" in proc.stdout
