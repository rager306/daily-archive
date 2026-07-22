"""Map hybrid batch selection rows to catalog coverage selection articles.

Pure application helper (no FS/network). Hybrid selection shape (M213/M214):
papers[].{paper_id, category, pdf_path, ...} → catalog selection articles
with article_ref = arxiv/{category}/{paper_id}.
"""

from __future__ import annotations

from typing import Any


def hybrid_paper_to_article_ref(*, paper_id: str, category: str) -> str:
    """Build canonical article_ref for arXiv catalog rows."""
    pid = paper_id.strip()
    cat = category.strip()
    if not pid or not cat:
        raise ValueError("paper_id and category required for article_ref")
    return f"arxiv/{cat}/{pid}"


def map_hybrid_selection_to_catalog_selection(
    hybrid_selection: dict[str, Any],
    *,
    selection_id: str | None = None,
) -> dict[str, Any]:
    """Convert hybrid gate selection JSON to catalog coverage selection shape.

    Invalid paper rows are still emitted with empty article_ref so the
    reconcilers can mark invalid_selection_ref (fail-closed honesty).
    """
    papers = hybrid_selection.get("papers")
    if not isinstance(papers, list):
        papers = []

    sid = selection_id
    if not sid:
        mid = hybrid_selection.get("milestone_id") or "hybrid-selection"
        rung = hybrid_selection.get("rung") or hybrid_selection.get("count") or "n"
        sid = f"hybrid-gate:{mid}:rung-{rung}"

    articles: list[dict[str, Any]] = []
    diagnostics: list[str] = []
    for raw in papers:
        if not isinstance(raw, dict):
            articles.append({"article_ref": "", "source_code": "arxiv"})
            diagnostics.append("invalid_paper_row_not_object")
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        category = str(raw.get("category") or "").strip()
        title = raw.get("title")
        if not paper_id or not category:
            articles.append(
                {
                    "article_ref": "",
                    "source_code": "arxiv",
                    "title": str(title) if title else None,
                    "paper_id": paper_id or None,
                    "category": category or None,
                }
            )
            diagnostics.append(f"invalid_paper_fields:{paper_id or '?'}")
            continue
        ref = hybrid_paper_to_article_ref(paper_id=paper_id, category=category)
        articles.append(
            {
                "article_ref": ref,
                "source_code": "arxiv",
                "title": str(title) if title else None,
                "paper_id": paper_id,
                "category": category,
                "pdf_path": raw.get("pdf_path"),
            }
        )

    return {
        "selection_id": str(sid),
        "articles": articles,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "diagnostics": diagnostics,
        "source_schema": str(hybrid_selection.get("schema_version") or ""),
        "paper_count": len(articles),
    }


__all__ = [
    "hybrid_paper_to_article_ref",
    "map_hybrid_selection_to_catalog_selection",
]
