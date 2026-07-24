"""M251 S02: operator Wave B gate script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_gate.py"


def test_no_stamp_blocked() -> None:
    """M254: explicit --no-stamp preserves M251 default-blocked contract."""
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is False
    assert report["gate_signal"] == "blocked"
    assert report["human_go"] is False
    assert report["import_eligible"] is False
    assert report["human_go_persisted"] is False


def test_human_go_dry_run_opens() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--human-go", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is True
    assert report["gate_signal"] == "open"
    assert report["human_go"] is True
    assert report["human_go_is_dry_run"] is True
    assert report["human_go_persisted"] is False
    assert report["import_eligible"] is False


def test_summary_line_no_stamp() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-gate" in proc.stdout
    assert "signal: blocked" in proc.stdout
    assert "import_eligible: false" in proc.stdout


def test_with_closeout_live_if_catalog() -> None:
    index = ROOT / "data" / "article_catalog" / "index.json"
    if not index.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--no-stamp", "--with-closeout", "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is False
    assert report["import_eligible"] is False
    # live A closed should show closeout context when metrics available
    if report.get("wave_a_closeout_pass") is True:
        assert report["wave_a_closeout_signal"] == "wave_a_closed"


def test_default_reads_repo_stamp_if_present() -> None:
    """M254: default path reads durable stamp (may be open on live repo)."""
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
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
    assert report["graph_writes_allowed"] is False
    if stamp.is_file():
        # live D124 stamp should open without dry-run
        if report.get("human_go") is True:
            assert report["wave_b_gate_open"] is True
            assert report["human_go_source"] == "stamp"
            assert report["human_go_persisted"] is True
            assert report["human_go_is_dry_run"] is False



def test_default_includes_closeout_when_catalog() -> None:
    """M257: default path loads live Wave A closeout (not stamp-only blind)."""
    index = ROOT / "data" / "article_catalog" / "index.json"
    if not index.is_file():
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
    # closeout context should be populated when catalog/bodies available
    assert report.get("wave_a_closeout_pass") is not None
    assert report.get("wave_a_closeout_signal") is not None
