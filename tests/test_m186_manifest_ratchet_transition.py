from __future__ import annotations

import json
from pathlib import Path
from typing import Any

CONTRACT = Path("data/architecture-assessment/m186-manifest-ratchet-transition-contract.json")


def _contract() -> dict[str, Any]:
    return json.loads(CONTRACT.read_text(encoding="utf-8"))


def test_m186_manifest_ratchet_current_mode_preserves_counts() -> None:
    payload = _contract()

    assert payload["current_mode"] == "preserve-ratchet"
    assert payload["current_required_counts"] == {
        "script-only": 4,
        "unknown": 0,
        "shared-state": 0,
    }


def test_m186_manifest_ratchet_blocks_residual_wiring_in_preserve_mode() -> None:
    preserve = _contract()["allowed_modes"]["preserve-ratchet"]

    assert preserve["residual_wiring_allowed"] is False
    assert preserve["required_counts"]["script-only"] == 4
    assert "lifecycle_contract_keeps_residual_blocked" in preserve["required_evidence"]


def test_m186_manifest_ratchet_transition_requires_explicit_baseline_update() -> None:
    transition = _contract()["allowed_modes"]["transition-ratchet"]

    assert transition["residual_wiring_allowed"] is True
    assert transition["required_counts"]["script-only"] == "explicit_new_baseline"
    assert "canonical_inventory_baseline_update" in transition["required_evidence"]
    assert "ratchet_decision_artifact" in transition["required_evidence"]
