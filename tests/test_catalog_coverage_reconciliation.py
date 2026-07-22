"""M210: selection-vs-catalog coverage reconciliation."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

import pytest

from research_graph.application.corpus.catalog_coverage_reconciliation import (
    CatalogCoveragePackage,
    CatalogCoverageReport,
    CatalogCoverageRow,
    build_catalog_coverage_package,
    reconcile_paths,
    reconcile_selection_against_catalog,
)
from research_graph.application.pipeline_continuity import (
    DEFAULT_LAYER_SEAMS,
    build_continuity_audit,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "src/research_graph/application/corpus/catalog_coverage_reconciliation.py"


def test_s01_types_and_fail_closed_report() -> None:
    report = CatalogCoverageReport(
        selection_id="sel-1",
        rows=(
            CatalogCoverageRow(
                article_ref="arxiv/cs-ai/1",
                status="cataloged",
                source_code="arxiv",
            ),
        ),
    )
    assert report.import_eligible is False
    assert report.graph_writes_allowed is False
    assert report.falkor_touched is False
    assert report.counts()["cataloged"] == 1
    with pytest.raises(ValueError, match="cannot authorize"):
        CatalogCoverageReport(
            selection_id="bad",
            rows=(),
            import_eligible=True,
        )


def test_s02_selection_vs_index_cataloged_and_missing_row() -> None:
    selection = {
        "selection_id": "sel-s02",
        "articles": [
            {"article_ref": "arxiv/cs-ai/ok", "source_code": "arxiv", "title": "OK"},
            {"article_ref": "arxiv/cs-ai/missing", "source_code": "arxiv", "title": "Missing"},
        ],
    }
    catalog_index = {
        "articles": [
            {
                "article_ref": "arxiv/cs-ai/ok",
                "source_code": "arxiv",
                "title": "OK",
                "article_path": "article_catalog/arxiv/cs-ai/ok/article.json",
            }
        ]
    }
    report = reconcile_selection_against_catalog(selection, catalog_index)
    by_ref = {row.article_ref: row for row in report.rows}
    assert by_ref["arxiv/cs-ai/ok"].status == "cataloged"
    assert by_ref["arxiv/cs-ai/ok"].blocker_code is None
    assert by_ref["arxiv/cs-ai/missing"].status == "missing_row"
    assert by_ref["arxiv/cs-ai/missing"].blocker_code == "typed_catalog_blocker"
    assert report.import_eligible is False
    assert len(report.blockers()) == 1


def test_s03_missing_article_json_not_already_cataloged() -> None:
    selection = {
        "selection_id": "sel-s03",
        "articles": [
            {"article_ref": "arxiv/cs-ai/ok", "source_code": "arxiv"},
            {"article_ref": "arxiv/cs-ai/nojson", "source_code": "arxiv"},
        ],
    }
    catalog_index = {
        "articles": [
            {"article_ref": "arxiv/cs-ai/ok", "source_code": "arxiv", "article_path": "ok.json"},
            {
                "article_ref": "arxiv/cs-ai/nojson",
                "source_code": "arxiv",
                "article_path": "nojson.json",
            },
        ]
    }

    def present(ref: str, row: Mapping[str, object]) -> bool:
        return ref == "arxiv/cs-ai/ok"

    report = reconcile_selection_against_catalog(
        selection, catalog_index, article_present=present
    )
    by_ref = {row.article_ref: row for row in report.rows}
    assert by_ref["arxiv/cs-ai/ok"].status == "cataloged"
    assert by_ref["arxiv/cs-ai/nojson"].status == "missing_article_json"
    assert by_ref["arxiv/cs-ai/nojson"].blocker_code == "missing_article_json"
    # never claim already_cataloged language
    assert all(row.status != "already_cataloged" for row in report.rows)  # type: ignore[comparison-overlap]


def test_s03_path_based_article_json_check(tmp_path: Path) -> None:
    article = tmp_path / "article_catalog" / "arxiv" / "cs-ai" / "ok" / "article.json"
    article.parent.mkdir(parents=True)
    article.write_text("{}", encoding="utf-8")
    selection = {
        "selection_id": "sel-path",
        "articles": [
            {"article_ref": "arxiv/cs-ai/ok", "source_code": "arxiv"},
            {"article_ref": "arxiv/cs-ai/gone", "source_code": "arxiv"},
        ],
    }
    catalog_index = {
        "articles": [
            {
                "article_ref": "arxiv/cs-ai/ok",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-ai/ok/article.json",
            },
            {
                "article_ref": "arxiv/cs-ai/gone",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-ai/gone/article.json",
            },
        ]
    }
    package = reconcile_paths(
        selection=selection,
        catalog_index=catalog_index,
        catalog_root=tmp_path,
        check_article_json=True,
    )
    by_ref = {row.article_ref: row for row in package.report.rows}
    assert by_ref["arxiv/cs-ai/ok"].status == "cataloged"
    assert by_ref["arxiv/cs-ai/gone"].status == "missing_article_json"
    assert package.verdict == "repair"


def test_s04_package_metadata_only_fail_closed() -> None:
    report = reconcile_selection_against_catalog(
        {
            "selection_id": "sel-s04",
            "articles": [{"article_ref": "a/1", "source_code": "arxiv"}],
        },
        {"articles": [{"article_ref": "a/1", "source_code": "arxiv"}]},
    )
    package = build_catalog_coverage_package(report)
    payload = package.to_dict()
    assert payload["import_eligible"] is False
    assert payload["graph_writes_allowed"] is False
    assert payload["safety_flags"] == SafetyFlags().to_dict()
    assert "api_key" not in str(payload).lower()
    with pytest.raises(ValueError, match="cannot authorize"):
        CatalogCoveragePackage(report=report, verdict="blocked", import_eligible=True)


def test_s05_continuity_seam_lists_reconciler() -> None:
    seams = DEFAULT_LAYER_SEAMS["source"] + DEFAULT_LAYER_SEAMS["review"]
    assert any("catalog_coverage_reconciliation.py" in s for s in seams)
    audit = build_continuity_audit(repo_root=ROOT)
    present = [p for layer in audit.layers for p in layer.present_seams]
    assert any("catalog_coverage_reconciliation.py" in p for p in present)
    assert MOD.exists()
    src = MOD.read_text(encoding="utf-8")
    assert "falkordb" not in src.casefold() or "must not touch" in src
    assert "import research_graph.infrastructure" not in src
