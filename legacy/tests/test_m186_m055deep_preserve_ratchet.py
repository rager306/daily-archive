from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
LIFECYCLE = ROOT / "data" / "architecture-assessment" / "m186-manifest-lifecycle-contract.json"
RATCHET = ROOT / "data" / "architecture-assessment" / "m186-manifest-ratchet-transition-contract.json"


def _json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_m055deep_residual_remains_blocked_under_preserve_ratchet() -> None:
    ratchet = _json(RATCHET)
    lifecycle = _json(LIFECYCLE)
    residual = next(row for row in lifecycle["residuals"] if row["id"] == "m055deep-20-pdf")

    assert ratchet["current_mode"] == "preserve-ratchet"
    assert ratchet["allowed_modes"]["preserve-ratchet"]["residual_wiring_allowed"] is False
    assert residual["status"] == "blocked"
    assert residual["atomicity"] is None
    assert residual["missing"] == ["owner", "invalidation", "consumer", "atomicity"]
