"""M236 S01: composition import-hold inventory helper."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    inventory_composition_import_hold,
)

ROOT = Path(__file__).resolve().parents[1]
COMPOSITION = ROOT / "src/research_graph/workflows/composition"


def test_inventory_lists_known_hold_surfaces() -> None:
    report = inventory_composition_import_hold(COMPOSITION)
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["enablement_hit_count"] == 0
    assert report["enablement_hits"] == []
    modules = set(report["modules_with_import_eligible"])
    assert "hybrid_readiness_handoff.py" in modules
    assert "non_arxiv_html_source_proof.py" in modules
    assert "graph_data_readiness.py" in modules
    assert report["module_count"] >= 3
    assert report["scanned_file_count"] >= report["module_count"]


def test_inventory_scan_is_deterministic() -> None:
    a = inventory_composition_import_hold(COMPOSITION)
    b = inventory_composition_import_hold(COMPOSITION)
    assert a["modules_with_import_eligible"] == b["modules_with_import_eligible"]
    assert a["enablement_hits"] == b["enablement_hits"]


def test_inventory_detects_enablement_in_fixture_dir(tmp_path: Path) -> None:
    bad = tmp_path / "bad_wire.py"
    bad.write_text("import_eligible = True\ngraph_writes_allowed = False\n", encoding="utf-8")
    ok = tmp_path / "ok_wire.py"
    ok.write_text("import_eligible = False\n", encoding="utf-8")
    report = inventory_composition_import_hold(tmp_path)
    assert report["enablement_hit_count"] == 1
    assert any("bad_wire.py" in h for h in report["enablement_hits"])
    assert report["import_eligible"] is False
