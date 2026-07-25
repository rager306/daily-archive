from __future__ import annotations

import json
from pathlib import Path
from typing import Any

CONTRACT = Path("data/architecture-assessment/m186-manifest-lifecycle-contract.json")
REQUIRED_DIMENSIONS = {"owner", "invalidation", "consumer", "atomicity", "lifecycle_tests"}
EXPECTED_RESIDUALS = {
    "scripts/benchmark_m055_corpus_manifest.py",
    "scripts/build_m055deep_corpus_manifest_20.py",
    "scripts/m058_build_graph_manifest.py",
    "scripts/m059_build_manifest.py",
}


def _contract() -> dict[str, Any]:
    return json.loads(CONTRACT.read_text(encoding="utf-8"))


def test_m186_manifest_lifecycle_contract_names_all_proof_dimensions() -> None:
    payload = _contract()

    assert set(payload["proof_dimensions"]) == REQUIRED_DIMENSIONS
    assert payload["movement_rule"] == "blocked_until_all_proof_dimensions_complete"


def test_m186_manifest_lifecycle_contract_maps_all_residuals() -> None:
    payload = _contract()
    residuals = payload["residuals"]

    assert {row["path"] for row in residuals} == EXPECTED_RESIDUALS
    assert len(residuals) == 4


def test_m186_manifest_lifecycle_contract_blocks_incomplete_residuals() -> None:
    payload = _contract()

    for row in payload["residuals"]:
        missing = set(row["missing"])
        assert missing
        assert missing <= REQUIRED_DIMENSIONS
        assert row["status"] == "blocked"
        assert row["lifecycle_tests"]
