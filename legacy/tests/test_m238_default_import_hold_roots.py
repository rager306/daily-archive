"""M238 S01: default package roots for import-hold inventory."""

from __future__ import annotations

from research_graph.application.corpus.composition_import_hold_inventory import (
    default_import_hold_roots,
    inventory_import_hold_trees,
)


def test_default_roots_are_four_existing_package_dirs() -> None:
    roots = default_import_hold_roots()
    assert len(roots) == 4
    names = [p.name for p in roots]
    # last path components for clarity
    assert names == ["domain", "application", "composition", "infrastructure"]
    for root in roots:
        assert root.is_dir(), root


def test_default_roots_inventory_is_clean() -> None:
    report = inventory_import_hold_trees(default_import_hold_roots())
    assert report["tree_count"] == 4
    assert report["enablement_hit_count"] == 0, report["enablement_hits"]
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    modules = set(report["modules_with_import_eligible"])
    assert any(m.endswith("hybrid_readiness_handoff.py") for m in modules)
    assert any(m.endswith("contracts.py") for m in modules)
    assert report["module_count"] >= 20


def test_default_roots_are_fresh_list() -> None:
    a = default_import_hold_roots()
    b = default_import_hold_roots()
    assert a is not b
    assert a == b
