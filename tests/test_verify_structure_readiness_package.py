"""Operator tests for structure readiness package (M262)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_structure_readiness_package.py"


def test_structure_readiness_operator_skip_etl() -> None:
    out = ROOT / "artifacts" / "etl" / "structure-readiness-test.json"
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--skip-etl-pack",
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
    assert report["falkor_touched"] is False
    assert "structure_signal" in report
    assert "structure_layer_health" in report
