"""Operator tests for Wave B stamp immutability (M257 S05)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_stamp_immutability.py"


def test_stamp_immutability_tmp(tmp_path: Path) -> None:
    from research_graph.application.corpus.wave_b_extraction_baseline import (
        write_human_go_stamp,
    )

    stamp = tmp_path / "human_go.json"
    first = write_human_go_stamp(
        stamp,
        authorized_by="user",
        decision_ref="D124",
        note="test",
        force_rewrite=True,
    )
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--stamp", str(stamp), "--json", "--strict"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["stamp_guard"] == "ok"
    assert report["authorized_at_unchanged"] is True
    assert report["import_eligible"] is False
    assert report["authorized_at"] == first["authorized_at"]


def test_missing_stamp_not_strict_ok(tmp_path: Path) -> None:
    missing = tmp_path / "nope.json"
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--stamp", str(missing), "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    report = json.loads(proc.stdout)
    assert report["stamp_guard"] == "missing_or_invalid"
