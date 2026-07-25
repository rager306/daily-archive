"""M246 S01: hybrid expand preflight (import-blocked, plan before batch)."""

from __future__ import annotations

from research_graph.application.corpus.hybrid_expand_preflight import (
    HybridExpandPreflightPackage,
    ProposedPaperCheck,
    derive_preflight_signal,
    preflight_hybrid_expand,
)


def test_signal_blocked_when_empty_proposal() -> None:
    assert (
        derive_preflight_signal(
            proposed_count=0,
            missing_pdf_count=0,
            already_bodied_count=0,
            ready_count=0,
        )
        == "blocked"
    )


def test_signal_repair_when_some_missing() -> None:
    assert (
        derive_preflight_signal(
            proposed_count=5,
            missing_pdf_count=2,
            already_bodied_count=0,
            ready_count=3,
        )
        == "repair"
    )


def test_signal_ready_when_all_pdfs_present_and_not_bodied() -> None:
    assert (
        derive_preflight_signal(
            proposed_count=5,
            missing_pdf_count=0,
            already_bodied_count=0,
            ready_count=5,
        )
        == "ready_to_batch"
    )


def test_preflight_marks_missing_and_bodied() -> None:
    checks = (
        ProposedPaperCheck(
            paper_id="p1",
            pdf_path="a.pdf",
            pdf_exists=True,
            already_bodied=False,
            byte_size=100,
        ),
        ProposedPaperCheck(
            paper_id="p2",
            pdf_path="b.pdf",
            pdf_exists=False,
            already_bodied=False,
            byte_size=100,
        ),
        ProposedPaperCheck(
            paper_id="p3",
            pdf_path="c.pdf",
            pdf_exists=True,
            already_bodied=True,
            byte_size=100,
        ),
    )
    pkg = preflight_hybrid_expand(
        checks=checks,
        selection_path="artifacts/m213-hybrid-gate/selection-40-proposal.json",
        target_count=20,
    )
    assert pkg.import_eligible is False
    assert pkg.proposed_count == 3
    assert pkg.missing_pdf_count == 1
    assert pkg.already_bodied_count == 1
    assert pkg.ready_count == 1
    assert pkg.preflight_signal == "repair"
    assert pkg.ready_paper_ids == ("p1",)
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["preflight_signal"] == "repair"


def test_preflight_ready_to_batch() -> None:
    checks = tuple(
        ProposedPaperCheck(
            paper_id=f"p{i}",
            pdf_path=f"p{i}.pdf",
            pdf_exists=True,
            already_bodied=False,
            byte_size=1000,
        )
        for i in range(3)
    )
    pkg = preflight_hybrid_expand(checks=checks, selection_path="sel.json", target_count=3)
    assert pkg.preflight_signal == "ready_to_batch"
    assert pkg.ready_count == 3


def test_rejects_import_true() -> None:
    import pytest

    with pytest.raises(ValueError):
        HybridExpandPreflightPackage(
            schema_version="x",
            preflight_signal="blocked",
            proposed_count=0,
            missing_pdf_count=0,
            already_bodied_count=0,
            ready_count=0,
            ready_paper_ids=(),
            missing_paper_ids=(),
            already_bodied_paper_ids=(),
            selection_path="",
            target_count=0,
            diagnostics=(),
            import_eligible=True,
        )
