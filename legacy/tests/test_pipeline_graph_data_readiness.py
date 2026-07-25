"""M209: pipeline continuity + no-write graph data readiness composition."""

from __future__ import annotations

import ast
from pathlib import Path

import pytest

from research_graph.workflows.composition.graph_data_readiness import (
    GraphDataReadinessPackage,
    GraphDataReadinessRequest,
    SourceInput,
    run_graph_data_readiness_pipeline,
)
from research_graph.application.pipeline_continuity import (
    PIPELINE_LAYERS,
    build_continuity_audit,
    render_continuity_report,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

ROOT = Path(__file__).resolve().parents[1]
APP_CONT = ROOT / "src/research_graph/application/pipeline_continuity.py"
APP_READY = ROOT / "src/research_graph/workflows/composition/graph_data_readiness.py"


@pytest.fixture()
def fixture_sources(tmp_path: Path) -> tuple[SourceInput, SourceInput]:
    html = tmp_path / "chapter.html"
    html.write_text(
        """<!doctype html><html><body>
        <h1>Graph Data Readiness Chapter</h1>
        <p>Local HTML content about PageIndex and evidence paths for readiness.</p>
        <h2>Method</h2>
        <p>Deterministic structure without network fetch or graph write.</p>
        <h2>Conclusion</h2>
        <p>source_kind must survive readiness packaging.</p>
        </body></html>
        """,
        encoding="utf-8",
    )
    md = tmp_path / "paper.md"
    md.write_text(
        """# Paper For Readiness

## Abstract
Local markdown is enough to build a deterministic PageIndex.

## Method
The agent builds structure from deterministic local markdown.

## Conclusion
Graph data readiness remains import-blocked.
""",
        encoding="utf-8",
    )
    return (
        SourceInput(path=str(html), paper_id="html-ready-1", source_type="html"),
        SourceInput(path=str(md), paper_id="paper-ready-1", source_type="markdown"),
    )


def test_s01_continuity_audit_seven_layers_fail_closed() -> None:
    audit = build_continuity_audit(repo_root=ROOT)
    assert len(audit.layers) == 7
    assert [layer.layer for layer in audit.layers] == list(PIPELINE_LAYERS)
    assert audit.import_eligible is False
    assert audit.graph_writes_allowed is False
    assert audit.falkor_touched is False
    assert audit.overall in {"partial", "present", "gap", "blocked"}
    # known post-M208 wiring gaps should surface
    codes = audit.gap_codes()
    assert any("composition_root_missing_pre_m209" in code or "cli_not_wired" in code for code in codes)
    md = render_continuity_report(audit)
    assert "Pipeline Continuity Audit" in md
    assert "import_eligible" in md


def test_s01_continuity_rejects_write_flags() -> None:
    with pytest.raises(ValueError, match="cannot authorize"):
        from research_graph.application.pipeline_continuity import ContinuityAudit, LayerStatus

        ContinuityAudit(
            layers=tuple(
                LayerStatus(
                    layer=name,
                    health="present",
                    present_seams=(),
                    missing_seams=(),
                    gaps=(),
                )
                for name in PIPELINE_LAYERS
            ),
            overall="present",
            import_eligible=True,
        )


def test_s02_s03_composition_root_html_and_markdown(fixture_sources) -> None:
    html, md = fixture_sources
    result = run_graph_data_readiness_pipeline(
        GraphDataReadinessRequest(
            sources=(html, md),
            review_completed=True,
            repo_root=str(ROOT),
            require_min_chunks=1,
        )
    )
    package = result.package
    assert package.falkor_touched is False
    assert package.import_eligible is False
    assert package.graph_writes_allowed is False
    assert package.production_import_attempted is False
    assert len(package.sources) == 2
    kinds = {source.source_kind for source in package.sources}
    assert "html" in kinds
    assert "markdown" in kinds
    assert all(source.load_ok for source in package.sources)
    assert all(source.structure_ok for source in package.sources)
    assert all(source.import_eligible is False for source in package.sources)
    assert package.verdict in {"ready_for_review", "repair", "blocked"}
    # with review completed and structure ok, expect ready_for_review
    assert package.verdict == "ready_for_review"
    assert all(source.pilot_eligible for source in package.sources)
    assert "Pipeline Continuity Audit" in result.continuity_report_markdown


def test_s04_readiness_package_metadata_only_no_leakage(fixture_sources) -> None:
    html, md = fixture_sources
    result = run_graph_data_readiness_pipeline(
        GraphDataReadinessRequest(sources=(html, md), repo_root=str(ROOT))
    )
    payload = result.package.to_dict()
    text = str(payload).lower()
    assert "api_key" not in text
    assert "embedding" not in text
    assert payload["counts"]["source_count"] == 2
    assert payload["verdict"] == "ready_for_review"
    assert payload["safety_flags"] == SafetyFlags().to_dict()


def test_s04_package_rejects_write_flags() -> None:
    audit = build_continuity_audit(repo_root=ROOT)
    with pytest.raises(ValueError, match="cannot authorize"):
        GraphDataReadinessPackage(
            sources=(),
            continuity=audit,
            verdict="blocked",
            import_eligible=True,
        )


def test_s05_fail_closed_and_no_falkor_imports() -> None:
    for path in (APP_CONT, APP_READY):
        src = path.read_text(encoding="utf-8")
        assert "falkordb" not in src.casefold() or "falkor_deferred" in src or "DisabledFalkor" in src
        # no live falkordb package import
        tree = ast.parse(src)
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    assert alias.name.split(".")[0] != "falkordb"
            if isinstance(node, ast.ImportFrom) and node.module:
                assert not node.module.startswith("falkordb")
                assert "pilot_write" not in node.module


def test_s05_failed_load_is_blocked_not_import_eligible(tmp_path: Path) -> None:
    missing = SourceInput(path=str(tmp_path / "nope.md"), paper_id="missing-1", source_type="markdown")
    result = run_graph_data_readiness_pipeline(
        GraphDataReadinessRequest(sources=(missing,), repo_root=str(ROOT))
    )
    source = result.package.sources[0]
    assert source.load_ok is False
    assert source.pilot_eligible is False
    assert source.import_eligible is False
    assert result.package.verdict in {"blocked", "repair"}


def test_s06_continuity_report_and_project_stage_mentions_post_m208() -> None:
    audit = build_continuity_audit(repo_root=ROOT)
    report = render_continuity_report(audit)
    assert "overall" in report
    project = (ROOT / ".gsd/PROJECT.md").read_text(encoding="utf-8")
    assert "post-M208" in project or "graph-data readiness" in project.casefold()
    assert "import" in project.casefold()
    assert "falkor" in project.casefold()
