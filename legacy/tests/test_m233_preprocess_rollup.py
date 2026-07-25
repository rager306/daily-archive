"""M233 S01: preprocess body rollup diagnostics (non-gating)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.preprocess_rollup import (
    rollup_preprocess_bodies,
)
from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)


def test_rollup_empty() -> None:
    r = rollup_preprocess_bodies([])
    assert r["body_count"] == 0
    assert r["import_eligible"] is False
    assert r["quality_status_counts"] == {}
    assert r["keyword_source_counts"] == {}


def test_rollup_counts_quality_and_keyword_source() -> None:
    rows = [
        {"quality_status": "ok", "keyword_source": "token_frequency"},
        {"quality_status": "ok", "keyword_source": "injected"},
        {"quality_status": "weak", "keyword_source": "token_frequency"},
        {"quality_status": "ok"},  # missing keyword_source → unknown
    ]
    r = rollup_preprocess_bodies(rows)
    assert r["body_count"] == 4
    assert r["quality_status_counts"] == {"ok": 3, "weak": 1}
    assert r["keyword_source_counts"] == {
        "token_frequency": 2,
        "injected": 1,
        "unknown": 1,
    }
    assert r["import_eligible"] is False
    assert r["drives_verdict"] is False


def _handoff(tmp_path: Path, bodies: dict[str, str]) -> HybridReadinessHandoffRequest:
    papers = []
    articles = []
    for pid, text in bodies.items():
        papers.append({"paper_id": pid, "category": "cs-cl", "pdf_path": f"{pid}.pdf"})
        articles.append(
            {
                "article_ref": f"arxiv/cs-cl/{pid}",
                "source_code": "arxiv",
                "article_path": f"article_catalog/arxiv/cs-cl/{pid}/article.json",
            }
        )
        art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / pid / "article.json"
        art.parent.mkdir(parents=True, exist_ok=True)
        art.write_text("{}", encoding="utf-8")
        body_path = tmp_path / "bodies" / pid / "body" / f"{pid}.hybrid.body.md"
        body_path.parent.mkdir(parents=True, exist_ok=True)
        body_path.write_text(text, encoding="utf-8")
    sel = {
        "schema_version": "hybrid-gate-selection.v1",
        "milestone_id": "M233-fixture",
        "count": len(papers),
        "papers": papers,
    }
    index = {"articles": articles}
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    return HybridReadinessHandoffRequest(
        hybrid_selection_path=sel_path,
        body_root=tmp_path / "bodies",
        catalog_index_path=idx_path,
        catalog_root=tmp_path,
        repo_root=tmp_path,
        review_completed=True,
    )


_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for structure readiness and preprocess rollup.
"""


def test_hybrid_emits_preprocess_rollup(tmp_path: Path) -> None:
    result = run_hybrid_readiness_handoff(
        _handoff(tmp_path, {"ok1": _BODY, "ok2": _BODY})
    )
    assert result.import_eligible is False
    assert result.preprocess_rollup is not None
    rollup = result.preprocess_rollup
    assert rollup["body_count"] == 2
    assert rollup["drives_verdict"] is False
    assert rollup["import_eligible"] is False
    assert "token_frequency" in rollup["keyword_source_counts"]
    assert any(d.startswith("preprocess_rollup_bodies:2") for d in result.diagnostics)
    payload = result.to_dict()
    assert payload["preprocess_rollup"]["body_count"] == 2
