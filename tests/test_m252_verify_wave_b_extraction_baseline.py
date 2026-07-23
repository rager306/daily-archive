"""M252 S02: operator Wave B extraction baseline script."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_extraction_baseline.py"


def test_script_stamp_only(tmp_path: Path) -> None:
    stamp = tmp_path / "human_go.json"
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp-path",
            str(stamp),
            "--stamp-only",
            "--decision-ref",
            "D124",
            "--json",
            "--repo-root",
            str(tmp_path),
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
    assert report["stamp"]["human_go"] is True
    assert report["stamp"]["decision_ref"] == "D124"
    assert stamp.is_file()
    raw = json.loads(stamp.read_text(encoding="utf-8"))
    assert raw["import_eligible"] is False


def test_script_live_baseline_if_fixtures() -> None:
    fixtures = ROOT / "artifacts" / "m072-reviewed-extraction-benchmark" / "fixtures"
    if not (fixtures / "train-gold.jsonl").is_file():
        return
    stamp = ROOT / "artifacts" / "wave-b" / "human_go.json"
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp-path",
            str(stamp),
            "--decision-ref",
            "D124",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "wave-b-extraction-baseline" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "dspy: false" in proc.stdout
    assert "gate:" in proc.stdout
    assert stamp.is_file()


def test_script_json_baseline_metrics() -> None:
    fixtures = ROOT / "artifacts" / "m072-reviewed-extraction-benchmark" / "fixtures"
    if not (fixtures / "train-gold.jsonl").is_file():
        return
    stamp = ROOT / "artifacts" / "wave-b" / "human_go-test.json"
    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--stamp-path",
            str(stamp),
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["baseline"]["train_case_count"] >= 1
    assert "entity_f1" in report["baseline"]["train_metrics"]
    assert report["baseline"]["import_eligible"] is False
    assert report["baseline"]["dspy_optimizer_enabled"] is False
