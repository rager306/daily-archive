"""ETL: hybrid-missing catalog papers vs local PDF readiness (Wave A expand queue)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    audit_hybrid_missing_pdf_readiness,
)


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def _body(root: Path, paper_id: str) -> None:
    p = root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("# body\n", encoding="utf-8")


def _pdf(catalog_root: Path, paper_id: str, topic: str = "cs-cl") -> Path:
    p = (
        catalog_root
        / "article_catalog"
        / "arxiv"
        / topic
        / paper_id
        / "source"
        / f"{paper_id}.pdf"
    )
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(b"%PDF-1.4 test")
    return p


def test_missing_hybrid_with_and_without_pdf(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(
        idx,
        [
            {
                "article_key": "has-body",
                "article_ref": "arxiv/cs-cl/has-body",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/has-body/article.json",
            },
            {
                "article_key": "with-pdf",
                "article_ref": "arxiv/cs-cl/with-pdf",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/with-pdf/article.json",
            },
            {
                "article_key": "no-pdf",
                "article_ref": "arxiv/cs-cl/no-pdf",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/no-pdf/article.json",
            },
        ],
    )
    body_root = tmp_path / "bodies"
    _body(body_root, "has-body")
    _pdf(tmp_path, "with-pdf")
    # article.json optional for path resolve
    for pid in ("has-body", "with-pdf", "no-pdf"):
        aj = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / pid / "article.json"
        aj.parent.mkdir(parents=True, exist_ok=True)
        aj.write_text("{}", encoding="utf-8")

    pkg = audit_hybrid_missing_pdf_readiness(
        catalog_index_path=idx,
        catalog_root=tmp_path,
        body_roots=(body_root,),
        sample_limit=10,
    )
    assert pkg.hybrid_missing_count == 2
    assert pkg.missing_with_local_pdf_count == 1
    assert pkg.missing_without_local_pdf_count == 1
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    d = pkg.to_dict()
    assert d["missing_with_local_pdf_count"] == 1
    assert d["expand_ready_sample"]
    assert d["import_eligible"] is False


def test_empty_catalog_fail_closed(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(idx, [])
    pkg = audit_hybrid_missing_pdf_readiness(
        catalog_index_path=idx,
        catalog_root=tmp_path,
        body_roots=(),
    )
    assert pkg.hybrid_missing_count == 0
    assert pkg.import_eligible is False
