"""M216: hybrid coverage + readiness handoff composition."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.workflows.composition.hybrid_readiness_handoff import (
    HybridReadinessHandoffRequest,
    resolve_hybrid_body_paths,
    run_hybrid_readiness_handoff,
)

ROOT = Path(__file__).resolve().parents[1]
SEL20 = ROOT / "artifacts" / "m213-hybrid-gate" / "selection-20.json"
BODY_ROOT = ROOT / "artifacts" / "m213-hybrid-gate" / "runs-live-20"
CATALOG_INDEX = ROOT / "data" / "article_catalog" / "index.json"
CATALOG_ROOT = ROOT / "data" / "article_catalog"


def test_resolve_hybrid_body_paths_found_and_missing(tmp_path: Path) -> None:
    sel = {
        "papers": [
            {"paper_id": "p1", "pdf_path": "a.pdf"},
            {"paper_id": "p2", "pdf_path": "b.pdf"},
        ]
    }
    body = tmp_path / "p1" / "body" / "p1.hybrid.body.md"
    body.parent.mkdir(parents=True)
    body.write_text("# P1\n\nbody\n", encoding="utf-8")
    rows = resolve_hybrid_body_paths(sel, body_root=tmp_path)
    by_id = {r.paper_id: r for r in rows}
    assert by_id["p1"].found is True
    assert by_id["p2"].found is False


def test_handoff_fixture_coverage_repair_and_partial_bodies(tmp_path: Path) -> None:
    # selection: one cataloged with body, one missing from index
    sel = {
        "schema_version": "hybrid-gate-selection.v1",
        "milestone_id": "M216-fixture",
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
Local hybrid body for readiness structure.

## Method
Deterministic markdown without network.

## Results
Enough text for structure and candidates.
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
    assert result.graph_writes_allowed is False
    assert result.bodies_found == 1
    assert result.bodies_missing == 1
    assert result.coverage.package.verdict == "repair"
    assert result.handoff_verdict == "repair"
    assert result.readiness is not None
    assert result.readiness.package.import_eligible is False
    assert out.is_file()
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["handoff_verdict"] == "repair"


def test_handoff_rejects_import_authorization() -> None:
    from research_graph.application.corpus.catalog_coverage_reconciliation import (
        CatalogCoveragePackage,
        CatalogCoverageReport,
    )
    from research_graph.workflows.composition.hybrid_catalog_coverage import (
        HybridCatalogCoverageResult,
    )
    from research_graph.workflows.composition.hybrid_readiness_handoff import (
        HybridReadinessHandoffResult,
    )

    coverage_report = CatalogCoverageReport(selection_id="x", rows=())
    coverage_pkg = CatalogCoveragePackage(report=coverage_report, verdict="blocked")
    # Build minimal coverage result via run is heavy; exercise constructor guard on handoff
    # by constructing with import_eligible True on handoff itself.
    with pytest.raises(ValueError, match="cannot authorize"):
        HybridReadinessHandoffResult(
            schema_version="x",
            handoff_verdict="blocked",
            coverage=HybridCatalogCoverageResult(
                schema_version="c",
                package=coverage_pkg,
                mapped_selection_id="x",
                paper_count=0,
                cataloged_count=0,
                blocker_count=0,
                hybrid_selection_path="s",
                catalog_index_path="i",
                output_path=None,
            ),
            readiness=None,
            body_resolutions=(),
            bodies_found=0,
            bodies_missing=0,
            import_eligible=True,
        )


@pytest.mark.skipif(
    not (SEL20.is_file() and BODY_ROOT.is_dir() and CATALOG_INDEX.is_file()),
    reason="real selection-20 / runs-live-20 / catalog index not present",
)
def test_real_selection20_handoff_smoke() -> None:
    # Keep smoke small: resolve all bodies, run handoff (readiness on 20 can be slower).
    # Use subset selection of 2 papers for speed while still real files.
    full = json.loads(SEL20.read_text(encoding="utf-8"))
    subset = dict(full)
    subset["papers"] = full["papers"][:2]
    subset["count"] = 2
    # write temp subset next to repo artifacts is avoided; use in-memory via tmp under /tmp
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        dpath = Path(d)
        sel_path = dpath / "sel2.json"
        sel_path.write_text(json.dumps(subset), encoding="utf-8")
        out = dpath / "handoff.json"
        result = run_hybrid_readiness_handoff(
            HybridReadinessHandoffRequest(
                hybrid_selection_path=sel_path,
                body_root=BODY_ROOT,
                catalog_index_path=CATALOG_INDEX,
                catalog_root=CATALOG_ROOT,
                output_path=out,
                repo_root=ROOT,
            )
        )
    assert result.bodies_found == 2
    assert result.bodies_missing == 0
    assert result.coverage.package.verdict == "covered"
    assert result.coverage.cataloged_count == 2
    assert result.readiness is not None
    assert result.readiness.package.import_eligible is False
    assert result.import_eligible is False
    assert result.handoff_verdict in {"ready_for_review", "repair", "blocked"}
    # With covered catalog + bodies, should not be blocked solely by coverage
    assert result.handoff_verdict != "blocked" or result.readiness.package.verdict == "blocked"
