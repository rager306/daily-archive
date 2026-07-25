"""M253 S02: operator Wave B hybrid extraction inventory script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    write_human_go_stamp,
)

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_hybrid_extraction_inventory.py"

_BODY = """# Graph Neural Networks for Structured Learning

## Abstract
Graph neural networks process graph-structured data using iterative message
passing between neighboring nodes with enough scholarly prose for inventory.
"""


def test_script_fixture_gate_and_hybrid(tmp_path: Path) -> None:
    stamp = tmp_path / "human_go.json"
    write_human_go_stamp(stamp, authorized_by="user", decision_ref="D124")
    body_root = tmp_path / "bodies"
    for pid in ("a1", "a2"):
        p = body_root / pid / "body" / f"{pid}.hybrid.body.md"
        p.parent.mkdir(parents=True)
        p.write_text(_BODY, encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp-path",
            str(stamp),
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
    assert report["dspy_optimizer_enabled"] is False
    assert report["gate"]["wave_b_gate_open"] is True
    assert report["hybrid_inventory"]["candidate_count"] == 2
    assert report["disagreement_inventory"]["train_case_count"] >= 1


def test_script_live_smoke_if_stamp_and_bodies() -> None:
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
    if not stamp.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-hybrid-extraction-inventory" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "dspy: false" in proc.stdout
    assert "gate:" in proc.stdout
    assert "hybrid_candidates:" in proc.stdout
