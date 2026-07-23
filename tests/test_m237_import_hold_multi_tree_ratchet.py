"""M237 S02: ratchet application + composition import-hold trees."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    inventory_import_hold_trees,
)

ROOT = Path(__file__).resolve().parents[1]
APPLICATION = ROOT / "src/research_graph/application"
COMPOSITION = ROOT / "src/research_graph/workflows/composition"


def test_application_and_composition_zero_enablements() -> None:
    report = inventory_import_hold_trees([APPLICATION, COMPOSITION])
    assert report["enablement_hit_count"] == 0, report["enablement_hits"]
    assert report["enablement_hits"] == []
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["tree_count"] == 2


def test_multi_tree_includes_hold_surfaces_from_both_roots() -> None:
    report = inventory_import_hold_trees([APPLICATION, COMPOSITION])
    modules = set(report["modules_with_import_eligible"])
    assert any(m.endswith("hybrid_readiness_handoff.py") for m in modules)
    assert any(m.endswith("non_arxiv_html_source_proof.py") for m in modules)
    assert any(m.endswith("graph_data_readiness.py") for m in modules)
    # application-layer corpus surfaces
    assert any("preprocess_rollup.py" in m for m in modules)
    assert any("composition_import_hold_inventory.py" in m for m in modules)


def test_schema_version_is_m237() -> None:
    report = inventory_import_hold_trees([APPLICATION, COMPOSITION])
    assert "m237-import-hold-inventory" in report["schema_version"]
    assert report["scanned_file_count"] >= report["module_count"] >= 15
