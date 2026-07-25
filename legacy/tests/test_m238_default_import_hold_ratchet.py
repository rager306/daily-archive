"""M238 S02: ratchet default package import-hold roots — zero True enablements."""

from __future__ import annotations

from research_graph.application.corpus.composition_import_hold_inventory import (
    default_import_hold_roots,
    inventory_import_hold_trees,
)


def test_default_roots_zero_enablements() -> None:
    report = inventory_import_hold_trees(default_import_hold_roots())
    assert report["enablement_hit_count"] == 0, report["enablement_hits"]
    assert report["enablement_hits"] == []
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["tree_count"] == 4


def test_default_roots_cover_hold_surfaces_across_layers() -> None:
    report = inventory_import_hold_trees(default_import_hold_roots())
    modules = set(report["modules_with_import_eligible"])
    # composition
    assert any(m.endswith("hybrid_readiness_handoff.py") for m in modules)
    assert any(m.endswith("non_arxiv_html_source_proof.py") for m in modules)
    # application
    assert any("preprocess_rollup.py" in m for m in modules)
    assert any("composition_import_hold_inventory.py" in m for m in modules)
    # domain
    assert any(m.endswith("contracts.py") for m in modules)
    # infrastructure (at least one surface)
    assert any(m.startswith("infrastructure/") for m in modules)
    assert report["module_count"] >= 30


def test_default_roots_schema_is_m237_multi_tree() -> None:
    report = inventory_import_hold_trees(default_import_hold_roots())
    assert "m237-import-hold-inventory" in report["schema_version"]
    assert report["scanned_file_count"] >= report["module_count"]
