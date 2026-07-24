"""Operator tests for ETL continuity pack dashboard."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_etl_continuity_pack.py"


def test_continuity_pack_operator_json_fail_closed() -> None:
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
    dash = report["dashboard"]
    assert "hybrid_found" in dash
    assert "expand_ready_frac" in dash
    assert "multi_root_divergent_content_count" in dash
    assert dash["import_eligible"] is False
