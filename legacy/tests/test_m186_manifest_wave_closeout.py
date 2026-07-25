from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CLOSEOUT = ROOT / "data" / "architecture-assessment" / "m186-manifest-wave-closeout.json"
LIFECYCLE = ROOT / "data" / "architecture-assessment" / "m186-manifest-lifecycle-contract.json"
RATCHET = ROOT / "data" / "architecture-assessment" / "m186-manifest-ratchet-transition-contract.json"
EXPECTED_RESIDUALS = {
    "m055-five-pdf",
    "m055deep-20-pdf",
    "m058-graph-manifest",
    "m059-batch-manifest",
}


def _json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_m186_manifest_wave_closeout_preserves_current_ratchet() -> None:
    closeout = _json(CLOSEOUT)
    ratchet = _json(RATCHET)

    assert closeout["active_mode"] == "preserve-ratchet"
    assert ratchet["current_mode"] == "preserve-ratchet"
    assert closeout["strict_counts"] == {"script-only": 4, "unknown": 0, "shared-state": 0}
    assert ratchet["allowed_modes"]["preserve-ratchet"]["residual_wiring_allowed"] is False


def test_m186_manifest_wave_closeout_all_residuals_are_no_move_and_blocked() -> None:
    closeout = _json(CLOSEOUT)
    lifecycle = _json(LIFECYCLE)
    closeout_residuals = {row["id"]: row for row in closeout["residuals"]}
    lifecycle_residuals = {row["id"]: row for row in lifecycle["residuals"]}

    assert set(closeout_residuals) == EXPECTED_RESIDUALS
    assert set(lifecycle_residuals) == EXPECTED_RESIDUALS
    for residual_id in EXPECTED_RESIDUALS:
        assert closeout_residuals[residual_id]["outcome"] == "no-move"
        assert lifecycle_residuals[residual_id]["status"] == "blocked"
        assert lifecycle_residuals[residual_id]["missing"]


def test_m186_manifest_wave_closeout_requires_transition_for_future_movement() -> None:
    closeout = _json(CLOSEOUT)
    required = set(closeout["future_movement_requires"])

    assert "transition-ratchet mode" in required
    assert "canonical inventory baseline update" in required
    assert "exact GitNexus impact" in required
    assert "ratchet decision artifact" in required
