"""M227 S02: scholarly preprocess enrichment on hybrid readiness handoff."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    run_hybrid_readiness_handoff,
)


def test_handoff_preprocess_bodies_for_found_only(tmp_path: Path) -> None:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M227-fixture",
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
    art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / "ok1" / "article.json"
    art.parent.mkdir(parents=True)
    art.write_text("{}", encoding="utf-8")
    body = tmp_path / "bodies" / "ok1" / "body" / "ok1.hybrid.body.md"
    body.parent.mkdir(parents=True)
    body.write_text(
        """# OK1 Paper

## Abstract
Local hybrid body for readiness structure and scholarly preprocess.

## Method
Deterministic markdown without network for graph neural networks.

## Results
Enough text for structure candidates language and outline signals.
""",
        encoding="utf-8",
    )
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    out = tmp_path / "handoff.json"
    result = run_hybrid_readiness_handoff(
        HybridReadinessHandoffRequest(
            hybrid_selection_path=sel_path,
            body_root=tmp_path / "bodies",
            catalog_index_path=idx_path,
            catalog_root=tmp_path,
            output_path=out,
            repo_root=tmp_path,
            review_completed=True,
        )
    )
    assert result.import_eligible is False
    assert result.handoff_verdict == "repair"
    assert result.bodies_found == 1
    assert len(result.preprocess_bodies) == 1
    row = result.preprocess_bodies[0]
    assert row["source_id"] == "ok1"
    assert row["import_eligible"] is False
    assert len(row["content_fingerprint_sha256"]) == 64
    assert row["language"]
    assert result.schema_version.startswith("m227")
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert len(payload["preprocess_bodies"]) == 1
