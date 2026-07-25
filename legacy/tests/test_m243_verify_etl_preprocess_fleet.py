"""M243 S02: operator script for preprocess fleet metrics."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_etl_preprocess_fleet.py"

_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for quality scoring and language detection.
"""


def test_script_temp_fixture(tmp_path: Path) -> None:
    body_root = tmp_path / "bodies"
    p = body_root / "p1" / "body" / "p1.hybrid.body.md"
    p.parent.mkdir(parents=True)
    p.write_text(_BODY, encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
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
    assert report["body_count"] == 1
    assert report["error_count"] == 0
    assert sum(report["quality_status_counts"].values()) == 1


def test_script_summary_line(tmp_path: Path) -> None:
    body_root = tmp_path / "bodies"
    p = body_root / "p1" / "body" / "p1.hybrid.body.md"
    p.parent.mkdir(parents=True)
    p.write_text(_BODY, encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "etl-preprocess-fleet" in proc.stdout
    assert "bodies: 1" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "yake: false" in proc.stdout


def test_live_smoke_if_bodies_present() -> None:
    body = ROOT / "artifacts" / "m213-hybrid-gate" / "runs-live-20"
    if not body.is_dir():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "import_eligible: false" in proc.stdout
    assert "etl-preprocess-fleet" in proc.stdout
