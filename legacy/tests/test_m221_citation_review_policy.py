"""M221 citation review policy pure + composition."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.citation_candidate_inventory import (
    CitationInventoryPackage,
    PaperCitationInventory,
    build_citation_inventory,
)
from research_graph.application.corpus.citation_review_policy import (
    CitationReviewThresholds,
    evaluate_citation_review_policy,
)
from research_graph.workflows.composition.citation_review_policy import (
    CitationReviewPolicyRequest,
    run_citation_review_policy,
)

ROOT = Path(__file__).resolve().parents[1]


def _pkg(
    *,
    paper_count: int = 2,
    citation_total: int = 100,
    with_title: int = 98,
    with_authors: int = 90,
    with_idno: int = 40,
    empty_title: int = 2,
    papers_with_file: int = 2,
) -> CitationInventoryPackage:
    # Minimal nested papers for package validity
    papers = tuple(
        PaperCitationInventory(
            paper_id=f"p{i}",
            citation_count=citation_total // max(paper_count, 1),
            with_title=with_title // max(paper_count, 1),
            with_authors=with_authors // max(paper_count, 1),
            with_idno=with_idno // max(paper_count, 1),
            with_date=0,
            with_venue=0,
            empty_title=empty_title // max(paper_count, 1),
            header_title_present=True,
            header_author_count=1,
        )
        for i in range(paper_count)
    )
    return CitationInventoryPackage(
        schema_version="test",
        paper_count=paper_count,
        papers_with_citations_file=papers_with_file,
        citation_total=citation_total,
        with_title=with_title,
        with_authors=with_authors,
        with_idno=with_idno,
        with_date=0,
        with_venue=0,
        empty_title=empty_title,
        papers=papers,
    )


def test_policy_ready_for_human_review() -> None:
    policy = evaluate_citation_review_policy(_pkg())
    assert policy.verdict == "ready_for_human_review"
    assert policy.import_eligible is False
    assert policy.graph_writes_allowed is False
    assert policy.review_required is True
    assert policy.to_dict()["import_eligible"] is False
    assert any("title_coverage_ok" in c for c in policy.checks)


def test_policy_repair_on_low_title() -> None:
    policy = evaluate_citation_review_policy(
        _pkg(with_title=50, empty_title=50),
        thresholds=CitationReviewThresholds(min_title_coverage=0.90),
    )
    assert policy.verdict == "repair"
    assert any("title_coverage" in d for d in policy.diagnostics)
    assert policy.import_eligible is False


def test_policy_blocked_on_empty_inventory() -> None:
    policy = evaluate_citation_review_policy(
        _pkg(paper_count=0, citation_total=0, with_title=0, with_authors=0, with_idno=0, empty_title=0, papers_with_file=0)
    )
    assert policy.verdict == "blocked"
    assert policy.import_eligible is False


def test_idno_advisory_by_default_not_hard_fail() -> None:
    """Live selection-20 idno ~0.40 — below 0.50 but advisory only."""
    policy = evaluate_citation_review_policy(
        _pkg(with_idno=40),
        thresholds=CitationReviewThresholds(
            min_idno_coverage_advisory=0.50,
            enforce_idno=False,
        ),
    )
    assert policy.verdict == "ready_for_human_review"
    assert any("idno_coverage_advisory" in d for d in policy.diagnostics)


def test_enforce_idno_causes_repair() -> None:
    policy = evaluate_citation_review_policy(
        _pkg(with_idno=40),
        thresholds=CitationReviewThresholds(
            min_idno_coverage_advisory=0.50,
            enforce_idno=True,
        ),
    )
    assert policy.verdict == "repair"


def test_policy_package_rejects_import_true() -> None:
    from research_graph.application.corpus.citation_review_policy import (
        DEFAULT_THRESHOLDS,
        CitationReviewPolicyPackage,
    )

    with pytest.raises(ValueError):
        CitationReviewPolicyPackage(
            schema_version="x",
            verdict="ready_for_human_review",
            thresholds=DEFAULT_THRESHOLDS,
            title_coverage=1.0,
            author_coverage=1.0,
            idno_coverage=1.0,
            empty_title_fraction=0.0,
            citations_file_fraction=1.0,
            paper_count=1,
            citation_total=1,
            checks=(),
            diagnostics=(),
            import_eligible=True,
        )


def test_composition_policy_over_fixture(tmp_path: Path) -> None:
    body = tmp_path / "p1" / "body"
    body.mkdir(parents=True)
    (body / "p1.hybrid.header.json").write_text(
        json.dumps({"title": "P1", "authors": [{"full_name": "A"}]}),
        encoding="utf-8",
    )
    rows = [
        {"title": f"C{i}", "authors": [{"full_name": "X"}], "idnos": {"DOI": f"10/{i}"}}
        for i in range(10)
    ]
    (body / "p1.hybrid.citations.jsonl").write_text(
        "\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8"
    )
    sel = tmp_path / "sel.json"
    sel.write_text(json.dumps({"papers": [{"paper_id": "p1"}]}), encoding="utf-8")
    out = tmp_path / "policy.json"
    result = run_citation_review_policy(
        CitationReviewPolicyRequest(
            hybrid_selection_path=sel,
            body_root=tmp_path,
            output_path=out,
            repo_root=tmp_path,
        )
    )
    assert result.import_eligible is False
    assert result.policy.verdict == "ready_for_human_review"
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["policy"]["verdict"] == "ready_for_human_review"
    assert payload["policy"]["review_required"] is True


def test_composition_live_scholarly_20_if_present() -> None:
    body_root = ROOT / "artifacts" / "m213-hybrid-gate" / "runs-live-scholarly-20"
    sel = ROOT / "artifacts" / "m213-hybrid-gate" / "selection-20.json"
    if not body_root.is_dir() or not sel.is_file():
        return
    if not any(body_root.rglob("*.hybrid.citations.jsonl")):
        return
    result = run_citation_review_policy(
        CitationReviewPolicyRequest(
            hybrid_selection_path=sel,
            body_root=body_root,
            output_path=None,
            repo_root=ROOT,
        )
    )
    assert result.import_eligible is False
    assert result.policy.verdict == "ready_for_human_review"
    assert result.policy.citation_total >= 800
    assert result.policy.review_required is True
    # inventory helper still pure
    assert build_citation_inventory is not None
