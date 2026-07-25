"""M241 S01: catalog vs hybrid body coverage audit (import-blocked)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.etl_body_coverage_audit import (
    audit_catalog_body_coverage,
)


def _write_index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps(
            {
                "schema_version": "article-catalog-index.v1",
                "articles": articles,
            },
            indent=2,
        ),
        encoding="utf-8",
    )


def test_empty_index_zero_counts(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _write_index(idx, [])
    pkg = audit_catalog_body_coverage(catalog_index_path=idx, body_roots=())
    assert pkg.article_count == 0
    assert pkg.hybrid_body_found == 0
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False


def test_counts_hybrid_bodies_under_body_roots(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _write_index(
        idx,
        [
            {
                "article_key": "a1",
                "article_ref": "arxiv/cs-cl/a1",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/a1/article.json",
            },
            {
                "article_key": "a2",
                "article_ref": "arxiv/cs-cl/a2",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/a2/article.json",
            },
            {
                "article_key": "blog1",
                "article_ref": "company_blog/cs-ir/blog1",
                "source_code": "company_blog",
                "article_path": "article_catalog/company_blog/cs-ir/blog1/article.json",
            },
        ],
    )
    # article json files optional for hybrid body counter
    body_root = tmp_path / "bodies"
    for pid in ("a1",):
        p = body_root / pid / "body" / f"{pid}.hybrid.body.md"
        p.parent.mkdir(parents=True)
        p.write_text("# body\n", encoding="utf-8")

    pkg = audit_catalog_body_coverage(
        catalog_index_path=idx,
        body_roots=(body_root,),
        catalog_root=tmp_path,
    )
    assert pkg.article_count == 3
    assert pkg.by_source_code.get("arxiv") == 2
    assert pkg.by_source_code.get("company_blog") == 1
    assert pkg.hybrid_body_found == 1
    assert pkg.hybrid_body_missing == 2  # a2 + blog1 (blog not expected hybrid)
    assert pkg.import_eligible is False
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["hybrid_body_found"] == 1
    assert "hybrid_body_fraction" in d


def test_paper_id_from_article_ref_and_key() -> None:
    from research_graph.application.corpus.etl_body_coverage_audit import (
        paper_id_for_article,
    )

    assert (
        paper_id_for_article(
            {"article_key": "x", "article_ref": "arxiv/cs-cl/1706.03762"}
        )
        == "1706.03762"
    )
    assert paper_id_for_article({"article_key": "only-key"}) == "only-key"


def test_rejects_import_eligible_true() -> None:
    import pytest
    from research_graph.application.corpus.etl_body_coverage_audit import (
        EtlBodyCoveragePackage,
    )

    with pytest.raises(ValueError):
        EtlBodyCoveragePackage(
            schema_version="x",
            article_count=0,
            by_source_code={},
            hybrid_body_found=0,
            hybrid_body_missing=0,
            article_json_found=0,
            article_json_missing=0,
            body_roots_scanned=0,
            gaps=(),
            samples=(),
            diagnostics=(),
            import_eligible=True,
        )
