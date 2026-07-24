"""Wrapper tests for verify_wave_b_gepa_constrained_spike script."""

from __future__ import annotations

import json
import runpy
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_wave_b_gepa_constrained_spike.py"


def test_verify_script_no_stamp_blocked(capsys: pytest.CaptureFixture[str]) -> None:
    argv = ["verify_wave_b_gepa_constrained_spike.py", "--no-stamp", "--json"]
    old = sys.argv
    try:
        sys.argv = argv
        try:
            runpy.run_path(str(SCRIPT), run_name="__main__")
        except SystemExit as exc:
            assert exc.code == 0
    finally:
        sys.argv = old
    out = capsys.readouterr().out
    payload = json.loads(out)
    assert payload["operator_status"] == "blocked_gate"
    assert payload["import_eligible"] is False
    assert payload["dspy_optimizer_enabled"] is False
    assert payload["gepa_ran"] is False
