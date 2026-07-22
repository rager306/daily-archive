"""M215: hybrid selection → catalog coverage mapper + composition."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.hybrid_selection_map import (
    hybrid_paper_to_article_ref,
    map_hybrid_selection_to_catalog_selection,
)
from research_graph.workflows.composition.hybrid_catalog_coverage import (
    HybridCatalogCoverageRequest,
    run_hybrid_catalog_coverage,
)

ROOT = Path(__file__).resolve().parents[1]
SEL20 = ROOT / "artifacts" / "m213-hybrid-gate" / "selection-20.json"
CATALOG_INDEX = ROOT / "data" / "article_catalog" / "index.json"
CATALOG_ROOT = ROOT / "data" / "article_catalog"


def test_mapper_article_ref_and_invalid_rows() -> None:
    assert hybrid_paper_to_article_ref(paper_id="1206.6423", category="cs-cl") == (
        "arxiv/cs-cl/1206.6423"
    )
    mapped = map_hybrid_selection_to_catalog_selection(
        {
            "schema_version": "m213-hybrid-gate-selection.v1",
            "milestone_id": "M213-test",
            "count": 2,
            "papers": [
                {"paper_id": "1206.6423", "category": "cs-cl", "pdf_path": "x.pdf"},
                {"paper_id": "", "category": "cs-cl"},
            ],
        }
    )
    assert mapped["import_eligible"] is False
    assert mapped["paper_count"] == 2
    assert mapped["articles"][0]["article_ref"] == "arxiv/cs-cl/1206.6423"
    assert mapped["articles"][1]["article_ref"] == ""
    assert any("invalid_paper_fields" in d for d in mapped["diagnostics"])


def test_composition_missing_row_fixture(tmp_path: Path) -> None:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M215-fixture",
        "count": 2,
        "papers": [
            {"paper_id": "ok1", "category": "cs-cl", "pdf_path": "a.pdf"},
            {"paper_id": "missing", "category": "cs-cl", "pdf_path": "b.pdf"},
        ],
    }
    index = {
        "articles": [
            {
                "article_ref": "arxiv/cs-cl/ok1",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/ok1/article.json",
            }
        ]
    }
    # materialize article.json for ok1
    art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / "ok1" / "article.json"
    art.parent.mkdir(parents=True)
    art.write_text("{}", encoding="utf-8")
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    out = tmp_path / "coverage.json"
    result = run_hybrid_catalog_coverage(
        HybridCatalogCoverageRequest(
            hybrid_selection_path=sel_path,
            catalog_index_path=idx_path,
            catalog_root=tmp_path,
            check_article_json=True,
            output_path=out,
            repo_root=tmp_path,
        )
    )
    assert result.paper_count == 2
    assert result.cataloged_count == 1
    assert result.blocker_count == 1
    assert result.package.verdict == "repair"
    assert out.is_file()
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    statuses = {r["article_ref"]: r["status"] for r in payload["package"]["report"]["rows"]}
    assert statuses["arxiv/cs-cl/ok1"] == "cataloged"
    assert statuses["arxiv/cs-cl/missing"] == "missing_row"


def test_result_rejects_import_flags() -> None:
    from research_graph.application.corpus.catalog_coverage_reconciliation import (
        CatalogCoverageReport,
    )

    with pytest.raises(ValueError, match="cannot authorize"):
        CatalogCoverageReport(selection_id="bad", rows=(), import_eligible=True)


def test_real_selection20_against_catalog_index() -> None:
    """Operational smoke: M214 selection-20 should be fully cataloged."""
    assert SEL20.is_file() and CATALOG_INDEX.is_file()
    result = run_hybrid_catalog_coverage(
        HybridCatalogCoverageRequest(
            hybrid_selection_path=SEL20,
            catalog_index_path=CATALOG_INDEX,
            catalog_root=CATALOG_ROOT,
            check_article_json=True,
            output_path=None,
            repo_root=ROOT,
        )
    )
    assert result.paper_count == 20
    assert result.cataloged_count == 20
    assert result.blocker_count == 0
    assert result.package.verdict == "covered"
    assert result.package.report.import_eligible is False
    assert result.package.report.graph_writes_allowed is False
