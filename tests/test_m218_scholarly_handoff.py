"""M218: GROBID header/citations wired into hybrid readiness handoff."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    resolve_scholarly_artifact_paths,
    run_hybrid_readiness_handoff,
)

ROOT = Path(__file__).resolve().parents[1]
M217_SMOKE = ROOT / "artifacts" / "m217-grobid-etl-smoke" / "1508.07909"


def test_resolve_scholarly_found_and_missing(tmp_path: Path) -> None:
    sel = {
        "papers": [
            {"paper_id": "p1", "category": "cs-cl"},
            {"paper_id": "p2", "category": "cs-cl"},
        ]
    }
    body = tmp_path / "p1" / "body"
    body.mkdir(parents=True)
    (body / "p1.hybrid.header.json").write_text(
        json.dumps({"title": "Hello", "import_eligible": False}), encoding="utf-8"
    )
    (body / "p1.hybrid.citations.jsonl").write_text(
        json.dumps({"title": "C1", "import_eligible": False}) + "\n"
        + json.dumps({"title": "C2", "import_eligible": False}) + "\n",
        encoding="utf-8",
    )
    rows = resolve_scholarly_artifact_paths(sel, body_root=tmp_path, load_counts=True)
    by_id = {r.paper_id: r for r in rows}
    assert by_id["p1"].header_found is True
    assert by_id["p1"].citations_found is True
    assert by_id["p1"].citation_count == 2
    assert by_id["p1"].header_title == "Hello"
    assert by_id["p2"].header_found is False
    assert by_id["p2"].citations_found is False
    assert by_id["p2"].citation_count == 0


def test_handoff_includes_scholarly_wrapper(tmp_path: Path) -> None:
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M218-fixture",
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
    art.parent.mkdir(parents=True)
    art.write_text("{}", encoding="utf-8")
    body_dir = tmp_path / "bodies" / "ok1" / "body"
    body_dir.mkdir(parents=True)
    (body_dir / "ok1.hybrid.body.md").write_text(
        "# OK1\n\n## Abstract\nBody for readiness.\n\n## Method\nLocal.\n\n## Results\nOk.\n",
        encoding="utf-8",
    )
    (body_dir / "ok1.hybrid.header.json").write_text(
        json.dumps(
            {
                "title": "OK1 Paper",
                "authors": [{"full_name": "A Author"}],
                "import_eligible": False,
                "graph_writes_allowed": False,
            }
        ),
        encoding="utf-8",
    )
    (body_dir / "ok1.hybrid.citations.jsonl").write_text(
        "\n".join(
            [
                json.dumps({"title": "Cite A", "import_eligible": False}),
                json.dumps({"title": "Cite B", "import_eligible": False}),
                json.dumps({"title": "Cite C", "import_eligible": False}),
            ]
        )
        + "\n",
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
        )
    )
    assert result.import_eligible is False
    assert result.schema_version.startswith("m218")
    sw = result.scholarly_wrapper
    assert sw["import_eligible"] is False
    assert sw["headers_found"] == 1
    assert sw["citations_files_found"] == 1
    assert sw["citation_total"] == 3
    assert sw["complete_wrapper_count"] == 1
    assert sw["per_paper"][0]["header_title"] == "OK1 Paper"
    assert any("citation_total:3" in d for d in result.diagnostics)
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["scholarly_wrapper"]["citation_total"] == 3
    assert payload["import_eligible"] is False


def test_handoff_missing_scholarly_is_honest(tmp_path: Path) -> None:
    """Body present but no GROBID artifacts → scholarly counts zero, not invented."""
    sel = {
        "schema_version": "m213-hybrid-gate-selection.v1",
        "milestone_id": "M218-missing",
        "count": 1,
        "papers": [{"paper_id": "onlybody", "category": "cs-cl", "pdf_path": "a.pdf"}],
    }
    index = {
        "articles": [
            {
                "article_ref": "arxiv/cs-cl/onlybody",
                "source_code": "arxiv",
                "article_path": "article_catalog/arxiv/cs-cl/onlybody/article.json",
            }
        ]
    }
    art = tmp_path / "article_catalog" / "arxiv" / "cs-cl" / "onlybody" / "article.json"
    art.parent.mkdir(parents=True)
    art.write_text("{}", encoding="utf-8")
    body_dir = tmp_path / "bodies" / "onlybody" / "body"
    body_dir.mkdir(parents=True)
    (body_dir / "onlybody.hybrid.body.md").write_text(
        "# Only Body\n\n## Abstract\nNo header cites.\n\n## Method\nX.\n\n## Results\nY.\n",
        encoding="utf-8",
    )
    sel_path = tmp_path / "sel.json"
    idx_path = tmp_path / "index.json"
    sel_path.write_text(json.dumps(sel), encoding="utf-8")
    idx_path.write_text(json.dumps(index), encoding="utf-8")
    result = run_hybrid_readiness_handoff(
        HybridReadinessHandoffRequest(
            hybrid_selection_path=sel_path,
            body_root=tmp_path / "bodies",
            catalog_index_path=idx_path,
            catalog_root=tmp_path,
            repo_root=tmp_path,
        )
    )
    assert result.bodies_found == 1
    assert result.scholarly_wrapper["headers_found"] == 0
    assert result.scholarly_wrapper["headers_missing"] == 1
    assert result.scholarly_wrapper["citation_total"] == 0
    assert result.scholarly_wrapper["complete_wrapper_count"] == 0
    assert result.import_eligible is False


def test_smoke_m217_artifacts_if_present() -> None:
    header = M217_SMOKE / "body" / "1508.07909.hybrid.header.json"
    cites = M217_SMOKE / "body" / "1508.07909.hybrid.citations.jsonl"
    if not header.is_file() or not cites.is_file():
        return
    sel = {
        "papers": [{"paper_id": "1508.07909", "category": "cs-cl", "pdf_path": "x.pdf"}]
    }
    # body_root is parent of paper_id dir
    body_root = M217_SMOKE.parent
    rows = resolve_scholarly_artifact_paths(sel, body_root=body_root, load_counts=True)
    assert len(rows) == 1
    assert rows[0].header_found is True
    assert rows[0].citations_found is True
    assert rows[0].citation_count >= 10
    assert rows[0].header_title
