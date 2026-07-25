"""M244 S01: Wave A continuity readiness package (import-blocked)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.etl_continuity_readiness import (
    build_continuity_readiness,
    derive_readiness_signal,
)


_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for quality scoring and language detection.
"""


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def _body(root: Path, paper_id: str, text: str = _BODY) -> None:
    p = root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def test_signal_blocked_no_hybrid() -> None:
    assert (
        derive_readiness_signal(
            hybrid_body_found=0,
            article_count=10,
            preprocess_body_count=0,
            preprocess_error_count=0,
            quality_ok=0,
            quality_soft=0,
            gaps=("no_hybrid_bodies_under_body_roots",),
        )
        == "blocked"
    )


def test_signal_repair_partial_coverage() -> None:
    assert (
        derive_readiness_signal(
            hybrid_body_found=5,
            article_count=100,
            preprocess_body_count=5,
            preprocess_error_count=0,
            quality_ok=4,
            quality_soft=1,
            gaps=("partial_hybrid_body_coverage",),
        )
        == "repair"
    )


def test_signal_ready_for_review_when_fleet_healthy() -> None:
    assert (
        derive_readiness_signal(
            hybrid_body_found=20,
            article_count=230,
            preprocess_body_count=20,
            preprocess_error_count=0,
            quality_ok=15,
            quality_soft=5,
            gaps=("partial_hybrid_body_coverage", "multi_root_hybrid_body_copies"),
        )
        == "ready_for_review"
    )


def test_build_continuity_temp_tree(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    arts = [
        {
            "article_key": f"p{i}",
            "article_ref": f"arxiv/cs-cl/p{i}",
            "source_code": "arxiv",
        }
        for i in range(1, 12)
    ]
    _index(idx, arts)
    body_root = tmp_path / "bodies"
    for i in range(1, 11):
        _body(body_root, f"p{i}")

    pkg = build_continuity_readiness(
        catalog_index_path=idx,
        catalog_root=tmp_path,
        body_roots=(body_root,),
    )
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.coverage.hybrid_body_found == 10
    assert pkg.preprocess.body_count == 10
    assert pkg.readiness_signal in {"ready_for_review", "repair"}
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["readiness_signal"] == pkg.readiness_signal
    assert "coverage" in d and "preprocess" in d


def test_rejects_import_true() -> None:
    import pytest
    from research_graph.application.corpus.etl_continuity_readiness import (
        EtlContinuityReadinessPackage,
    )
    from research_graph.application.corpus.etl_body_coverage_audit import (
        EtlBodyCoveragePackage,
    )
    from research_graph.application.corpus.etl_preprocess_fleet_audit import (
        EtlPreprocessFleetPackage,
    )

    cov = EtlBodyCoveragePackage(
        schema_version="c",
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
    )
    pre = EtlPreprocessFleetPackage(
        schema_version="p",
        body_count=0,
        error_count=0,
        quality_status_counts={},
        language_counts={},
        keyword_source_counts={},
        samples=(),
        diagnostics=(),
    )
    with pytest.raises(ValueError):
        EtlContinuityReadinessPackage(
            schema_version="x",
            readiness_signal="blocked",
            coverage=cov,
            preprocess=pre,
            diagnostics=(),
            import_eligible=True,
        )
