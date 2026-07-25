"""M254 S01: stamp-aware Wave B gate operator (debt closeout)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    write_human_go_stamp,
)

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_gate.py"


def _run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=cwd or ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def test_no_stamp_flag_blocks_even_if_repo_stamp_exists() -> None:
    """--no-stamp must ignore durable stamp (operator false-positive guard)."""
    proc = _run("--no-stamp", "--json")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is False
    assert report["gate_signal"] == "blocked"
    assert report["human_go"] is False
    assert report["import_eligible"] is False
    assert report["human_go_source"] in {"none", "flag"}
    assert report.get("stamp_present") is False or report.get("human_go_source") != "stamp"


def test_tmp_stamp_opens_gate(tmp_path: Path) -> None:
    stamp = tmp_path / "human_go.json"
    write_human_go_stamp(stamp, authorized_by="user", decision_ref="D124")
    proc = _run("--stamp", str(stamp), "--json", "--repo-root", str(tmp_path))
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is True
    assert report["gate_signal"] == "open"
    assert report["human_go"] is True
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["stamp_present"] is True
    assert report["human_go_source"] == "stamp"
    assert report["human_go_persisted"] is True
    assert report["human_go_is_dry_run"] is False


def test_missing_explicit_stamp_blocked(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"
    proc = _run("--stamp", str(missing), "--json", "--repo-root", str(tmp_path))
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is False
    assert report["gate_signal"] == "blocked"
    assert report["import_eligible"] is False
    assert report["stamp_present"] is False
    assert report["human_go_source"] == "none"


def test_human_go_flag_with_no_stamp_is_dry_run() -> None:
    proc = _run("--no-stamp", "--human-go", "--json")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["wave_b_gate_open"] is True
    assert report["human_go_is_dry_run"] is True
    assert report["human_go_persisted"] is False
    assert report["human_go_source"] == "flag"
    assert report["import_eligible"] is False


def test_summary_line_mentions_source() -> None:
    proc = _run("--no-stamp")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-gate" in proc.stdout
    assert "import_eligible: false" in proc.stdout
