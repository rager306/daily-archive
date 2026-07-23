"""M245 S01: hybrid selection expand planner (import-blocked, no batch)."""

from __future__ import annotations

from research_graph.application.corpus.hybrid_selection_expand import (
    HybridSelectionExpandPackage,
    InventoryPdfRow,
    plan_next_hybrid_selection,
)


def _row(
    paper_id: str,
    category: str = "cs-cl",
    *,
    byte_size: int = 100_000,
    pdf_path: str | None = None,
    sha256: str = "a" * 64,
) -> InventoryPdfRow:
    return InventoryPdfRow(
        paper_id=paper_id,
        category=category,
        pdf_path=pdf_path or f"data/article_catalog/article_catalog/arxiv/{category}/{paper_id}/source/{paper_id}.pdf",
        byte_size=byte_size,
        sha256=sha256,
    )


def test_excludes_existing_selection_and_bodies() -> None:
    inventory = (
        _row("p1"),
        _row("p2"),
        _row("p3"),
        _row("p4"),
    )
    pkg = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=frozenset({"p1", "p2"}),
        target_count=10,
        max_bytes=25 * 1024 * 1024,
    )
    ids = {p.paper_id for p in pkg.proposed_papers}
    assert "p1" not in ids and "p2" not in ids
    assert ids == {"p3", "p4"}
    assert pkg.import_eligible is False
    assert pkg.proposed_count == 2


def test_size_cap_skips_large_pdfs() -> None:
    inventory = (
        _row("small", byte_size=1_000),
        _row("huge", byte_size=30 * 1024 * 1024),
    )
    pkg = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=frozenset(),
        target_count=5,
        max_bytes=25 * 1024 * 1024,
    )
    assert [p.paper_id for p in pkg.proposed_papers] == ["small"]
    assert "size_cap_skipped" in " ".join(pkg.diagnostics)


def test_category_diversity_prefers_new_categories() -> None:
    inventory = (
        _row("a1", "cs-cl"),
        _row("a2", "cs-cl"),
        _row("b1", "cs-ai"),
        _row("c1", "cs-cv"),
    )
    pkg = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=frozenset(),
        target_count=3,
        max_bytes=25 * 1024 * 1024,
    )
    cats = {p.category for p in pkg.proposed_papers}
    assert len(pkg.proposed_papers) == 3
    # should pick one from each category first
    assert cats == {"cs-cl", "cs-ai", "cs-cv"}


def test_target_count_limit() -> None:
    inventory = tuple(_row(f"p{i}", "cs-cl" if i % 2 == 0 else "cs-ai") for i in range(20))
    pkg = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=frozenset(),
        target_count=5,
        max_bytes=25 * 1024 * 1024,
    )
    assert pkg.proposed_count == 5
    assert pkg.target_count == 5


def test_to_selection_dict_shape() -> None:
    inventory = (_row("p9", "cs-ai", sha256="b" * 64),)
    pkg = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=frozenset(),
        target_count=5,
        max_bytes=25 * 1024 * 1024,
        rung=30,
        extends="artifacts/m213-hybrid-gate/selection-20.json",
        milestone_id="M245-test",
    )
    sel = pkg.to_selection_dict()
    assert sel["schema_version"] == "m213-hybrid-gate-selection.v1"
    assert sel["import_eligible"] is False
    assert sel["graph_writes_allowed"] is False
    assert sel["count"] == 1
    assert sel["rung"] == 30
    assert sel["papers"][0]["paper_id"] == "p9"
    assert len(sel["papers"][0]["sha256"]) == 64
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["proposed_count"] == 1


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        HybridSelectionExpandPackage(
            schema_version="x",
            proposed_count=0,
            target_count=5,
            available_after_filters=0,
            inventory_count=0,
            excluded_count=0,
            max_bytes=1,
            proposed_papers=(),
            diagnostics=(),
            selection_policy="p",
            extends="",
            milestone_id="m",
            rung=30,
            import_eligible=True,
        )
