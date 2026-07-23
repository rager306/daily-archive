"""M237 S01: multi-tree import-hold inventory and precise enablement scan."""

from __future__ import annotations

from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    inventory_composition_import_hold,
    inventory_import_hold_trees,
)

ROOT = Path(__file__).resolve().parents[1]
COMPOSITION = ROOT / "src/research_graph/workflows/composition"
APPLICATION = ROOT / "src/research_graph/application"


def test_string_and_doc_markers_are_not_enablements(tmp_path: Path) -> None:
    (tmp_path / "docs_only.py").write_text(
        '''"""does not set import_eligible=true and does not authorize"""\n'''
        'MARKERS = ("import_eligible: true", "graph_writes_allowed: true")\n'
        "import_eligible = False\n",
        encoding="utf-8",
    )
    report = inventory_import_hold_trees([tmp_path])
    assert report["enablement_hit_count"] == 0
    assert report["enablement_hits"] == []
    assert any(m.endswith("docs_only.py") for m in report["modules_with_import_eligible"])


def test_python_true_assignment_is_detected(tmp_path: Path) -> None:
    (tmp_path / "bad.py").write_text(
        "class X:\n    import_eligible = True\n    graph_writes_allowed = False\n",
        encoding="utf-8",
    )
    report = inventory_import_hold_trees([tmp_path])
    assert report["enablement_hit_count"] == 1
    assert any("bad.py" in h for h in report["enablement_hits"])


def test_multi_tree_real_application_and_composition_clean() -> None:
    report = inventory_import_hold_trees([APPLICATION, COMPOSITION])
    assert report["import_eligible"] is False
    assert report["graph_writes_allowed"] is False
    assert report["enablement_hit_count"] == 0, report["enablement_hits"]
    assert report["tree_count"] == 2
    modules = set(report["modules_with_import_eligible"])
    assert any(m.endswith("hybrid_readiness_handoff.py") for m in modules)
    assert any("preprocess_rollup.py" in m or m.endswith("preprocess_rollup.py") for m in modules) or any(
        "body_quality.py" in m for m in modules
    )
    assert report["scanned_file_count"] >= report["module_count"] >= 10


def test_composition_wrapper_still_works() -> None:
    report = inventory_composition_import_hold(COMPOSITION)
    assert report["enablement_hit_count"] == 0
    assert "hybrid_readiness_handoff.py" in report["modules_with_import_eligible"]
