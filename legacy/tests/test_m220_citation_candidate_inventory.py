"""M220 pure citation candidate inventory + composition scan."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.citation_candidate_inventory import (
    build_citation_inventory,
    inventory_paper_citations,
)
from research_graph.workflows.composition.citation_candidate_inventory import (
    CitationInventoryRequest,
    run_citation_candidate_inventory,
)


def test_inventory_paper_coverage() -> None:
    header = {
        "title": "Main Paper",
        "authors": [{"full_name": "A"}, {"full_name": "B"}],
    }
    cites = [
        {"title": "T1", "authors": [{"full_name": "X"}], "date": "2020", "idnos": {"DOI": "10.1/x"}},
        {"title": "", "authors": [], "date": "", "idnos": {}},
        {"title": "T3", "authors": [{"surname": "Y"}], "venue_or_monogr": "ACL"},
    ]
    inv = inventory_paper_citations(paper_id="p1", header=header, citations=cites)
    assert inv.citation_count == 3
    assert inv.with_title == 2
    assert inv.empty_title == 1
    assert inv.with_authors == 2
    assert inv.with_idno == 1
    assert inv.with_date == 1
    assert inv.with_venue == 1
    assert inv.header_title_present is True
    assert inv.header_author_count == 2
    assert inv.import_eligible is False
    d = inv.to_dict()
    assert d["title_coverage"] == pytest.approx(2 / 3)
    assert d["import_eligible"] is False


def test_build_inventory_aggregate_fail_closed() -> None:
    pkg = build_citation_inventory(
        [
            {
                "paper_id": "a",
                "header": {"title": "A", "authors": []},
                "citations": [{"title": "C1"}, {"title": "C2"}],
            },
            {
                "paper_id": "b",
                "header": None,
                "citations": None,
                "has_citations_file": False,
            },
        ]
    )
    assert pkg.paper_count == 2
    assert pkg.citation_total == 2
    assert pkg.with_title == 2
    assert pkg.papers_with_citations_file == 1
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.to_dict()["import_eligible"] is False
    with pytest.raises(ValueError):
        # construct would fail if forced true — package enforces via __post_init__
        from research_graph.application.corpus.citation_candidate_inventory import (
            CitationInventoryPackage,
        )

        CitationInventoryPackage(
            schema_version="x",
            paper_count=0,
            papers_with_citations_file=0,
            citation_total=0,
            with_title=0,
            with_authors=0,
            with_idno=0,
            with_date=0,
            with_venue=0,
            empty_title=0,
            papers=(),
            import_eligible=True,
        )


def test_composition_inventory_over_body_root(tmp_path: Path) -> None:
    # paper with full scholarly artifacts
    body = tmp_path / "p1" / "body"
    body.mkdir(parents=True)
    (body / "p1.hybrid.header.json").write_text(
        json.dumps({"title": "P1", "authors": [{"full_name": "A"}]}),
        encoding="utf-8",
    )
    (body / "p1.hybrid.citations.jsonl").write_text(
        "\n".join(
            [
                json.dumps({"title": "C1", "authors": [{"full_name": "X"}], "idnos": {"DOI": "10/x"}}),
                json.dumps({"title": "C2"}),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    # paper missing scholarly
    (tmp_path / "p2" / "body").mkdir(parents=True)
    (tmp_path / "p2" / "body" / "p2.hybrid.body.md").write_text("# only body\n", encoding="utf-8")

    sel = {
        "papers": [
            {"paper_id": "p1"},
            {"paper_id": "p2"},
        ]
    }
    sel_path = tmp_path / "sel.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    out = tmp_path / "inventory.json"
    result = run_citation_candidate_inventory(
        CitationInventoryRequest(
            hybrid_selection_path=sel_path,
            body_root=tmp_path,
            output_path=out,
            repo_root=tmp_path,
        )
    )
    assert result.import_eligible is False
    assert result.package.paper_count == 2
    assert result.package.citation_total == 2
    assert result.package.with_title == 2
    assert result.package.with_idno == 1
    assert result.package.papers_with_citations_file == 1
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["package"]["citation_total"] == 2


def test_composition_live_scholarly_10_if_present() -> None:
    root = Path(__file__).resolve().parents[1]
    body_root = root / "artifacts" / "m213-hybrid-gate" / "runs-live-scholarly"
    sel = root / "artifacts" / "m213-hybrid-gate" / "selection.json"
    if not body_root.is_dir() or not sel.is_file():
        return
    if not any(body_root.rglob("*.hybrid.citations.jsonl")):
        return
    result = run_citation_candidate_inventory(
        CitationInventoryRequest(
            hybrid_selection_path=sel,
            body_root=body_root,
            output_path=None,
            repo_root=root,
        )
    )
    assert result.import_eligible is False
    assert result.package.citation_total >= 100
    assert result.package.with_title > 0
    d = result.package.to_dict()
    assert d["title_coverage"] > 0.5
