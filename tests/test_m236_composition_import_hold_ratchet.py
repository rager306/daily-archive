"""M236 S02: ratchet composition import-hold — no True enablements."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    inventory_composition_import_hold,
)

ROOT = Path(__file__).resolve().parents[1]
COMPOSITION = ROOT / "src/research_graph/workflows/composition"

_REQUIRED = frozenset(
    {
        "hybrid_readiness_handoff.py",
        "non_arxiv_html_source_proof.py",
        "graph_data_readiness.py",
        "hybrid_batch_gate.py",
        "hybrid_catalog_coverage.py",
    }
)


def test_composition_has_zero_import_enablements() -> None:
    report = inventory_composition_import_hold(COMPOSITION)
    assert report["enablement_hit_count"] == 0, report["enablement_hits"]
    assert report["enablement_hits"] == []
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False


def test_required_hold_surfaces_are_inventoried() -> None:
    report = inventory_composition_import_hold(COMPOSITION)
    modules = set(report["modules_with_import_eligible"])
    missing = _REQUIRED - modules
    assert not missing, f"missing hold surfaces: {sorted(missing)}"


def test_inventory_schema_version_stable() -> None:
    report = inventory_composition_import_hold(COMPOSITION)
    assert "composition-import-hold-inventory" in report["schema_version"]
    assert report["scanned_file_count"] >= report["module_count"] >= len(_REQUIRED)
