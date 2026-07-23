"""M233 S02: preprocess enrichment must not gate handoff verdict or import."""

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


_STRONG = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks with
multiple baselines and ablation studies across domains.

## Results
Enough scholarly prose for structure readiness and preprocess rollup checks.
"""

# Intentionally short / noisy — quality may be weak, but must not open import.
_WEAK = "hi\n"


def _req(
    tmp_path: Path,
    body: str,
    *,
    use_yake_keywords: bool = False,
) -> HybridReadinessHandoffRequest:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M233-ratchet",
        "count": 1,
        "papers": [{"paper_id": "ok1", "category": "cs-cl", "pdf_path": "a.pdf"}],
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
    art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / "ok1" / "article.json"
    art.parent.mkdir(parents=True, exist_ok=True)
    art.write_text("{}", encoding="utf-8")
    body_path = tmp_path / "bodies" / "ok1" / "body" / "ok1.hybrid.body.md"
    body_path.parent.mkdir(parents=True, exist_ok=True)
    body_path.write_text(body, encoding="utf-8")
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
        use_yake_keywords=use_yake_keywords,
    )


def test_yake_flag_does_not_change_verdict_or_import(tmp_path: Path) -> None:
    off = run_hybrid_readiness_handoff(
        _req(tmp_path, _STRONG, use_yake_keywords=False)
    )
    on = run_hybrid_readiness_handoff(
        _req(tmp_path, _STRONG, use_yake_keywords=True)
    )
    assert off.import_eligible is False
    assert on.import_eligible is False
    assert off.handoff_verdict == on.handoff_verdict
    assert off.preprocess_bodies[0]["keyword_source"] == "token_frequency"
    assert on.preprocess_bodies[0]["keyword_source"] == "injected"
    assert off.preprocess_rollup["drives_verdict"] is False
    assert on.preprocess_rollup["drives_verdict"] is False


def test_weak_quality_body_cannot_authorize_import(tmp_path: Path) -> None:
    result = run_hybrid_readiness_handoff(
        _req(tmp_path, _WEAK, use_yake_keywords=False)
    )
    assert result.import_eligible is False
    assert result.preprocess_rollup["import_eligible"] is False
    assert result.preprocess_rollup["drives_verdict"] is False
    assert len(result.preprocess_bodies) == 1
    assert result.preprocess_bodies[0]["import_eligible"] is False


def test_rollup_always_non_gating() -> None:
    r = rollup_preprocess_bodies(
        [
            {"quality_status": "fail", "keyword_source": "injected"},
            {"quality_status": "ok", "keyword_source": "token_frequency"},
        ]
    )
    assert r["body_count"] == 2
    assert r["drives_verdict"] is False
    assert r["import_eligible"] is False
