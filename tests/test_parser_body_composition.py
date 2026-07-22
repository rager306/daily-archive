"""M211: parser body route policy + composition."""

from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.parser_body_route import (
    BODY_RESOLVE_RESOURCE_PROFILE,
    BODY_RESOLVE_STAGE_NAME,
    BodyRouteDecision,
    BodyRouteIntent,
    decide_body_route,
)
from research_graph.domain.ports import ConversionResult
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    resolve_article_body,
)
from research_graph.workflows.composition.single_article_pipeline import (
    SingleArticleRunRequest,
    run_single_article_pipeline,
)

ROOT = Path(__file__).resolve().parents[1]


def test_s01_decide_body_route_auto_and_preferences() -> None:
    html = decide_body_route(
        BodyRouteIntent(preference="auto", has_local_html=True)
    )
    assert html.route == "html_native"
    assert html.hybrid_claimed_success is False

    md = decide_body_route(
        BodyRouteIntent(
            preference="mdconverter",
            has_arxiv_id=True,
            fulltext_provider_available=True,
        )
    )
    assert md.route == "mdconverter"

    hybrid = decide_body_route(BodyRouteIntent(preference="hybrid"))
    assert hybrid.route == "hybrid_deferred"
    assert hybrid.hybrid_claimed_success is False
    assert "do_not_claim_hybrid_success" in hybrid.diagnostics

    fitz = decide_body_route(
        BodyRouteIntent(preference="fitz", has_local_pdf=True, fitz_fallback_allowed=True)
    )
    assert fitz.route == "fitz_offline"


def test_s01_policy_forbids_hybrid_success_flag() -> None:
    with pytest.raises(ValueError, match="forbids hybrid_claimed_success"):
        BodyRouteDecision(
            route="hybrid_deferred",
            reason="x",
            hybrid_claimed_success=True,
        )


def test_s01_resource_seam_markers() -> None:
    assert BODY_RESOLVE_STAGE_NAME == "parser_body_resolve"
    assert BODY_RESOLVE_RESOURCE_PROFILE.io_required is True
    assert BODY_RESOLVE_RESOURCE_PROFILE.llm_required is False
    d = decide_body_route(BodyRouteIntent(has_local_html=True))
    payload = d.to_dict()
    assert payload["stage_name"] == BODY_RESOLVE_STAGE_NAME
    assert payload["resource_profile"]["io_required"] is True


def test_s02_resolve_local_html_body(tmp_path: Path) -> None:
    html = tmp_path / "a.html"
    html.write_text(
        "<html><body><h1>T</h1><p>Body for structure path.</p>"
        "<h2>M</h2><p>More text here.</p></body></html>",
        encoding="utf-8",
    )
    result = resolve_article_body(
        ArticleBodyRequest(source=str(html), work_dir=tmp_path / "w", preference="auto")
    )
    assert result.route == "html_native"
    assert result.body_path is not None
    assert result.body_chars > 0
    assert result.decision.hybrid_claimed_success is False


class _FakeFullText:
    def convert_sync(self, arxiv_id: str) -> ConversionResult:
        return ConversionResult(
            markdown=f"# {arxiv_id}\n\n## Intro\n\nConverted body for readiness.\n\n## Out\n\nDone.\n",
            method="fake",
            error=None,
        )


def test_s02_mdconverter_path_with_fake_provider(tmp_path: Path) -> None:
    result = resolve_article_body(
        ArticleBodyRequest(
            source="2607.13104v1",
            work_dir=tmp_path / "w",
            preference="mdconverter",
            allow_network=False,
        ),
        fulltext_provider=_FakeFullText(),
    )
    assert result.route == "mdconverter"
    assert result.body_path is not None
    assert "Converted body" in result.body_path.read_text(encoding="utf-8")
    assert "not_hybrid" in result.diagnostics


def test_s02_hybrid_deferred_no_body(tmp_path: Path) -> None:
    result = resolve_article_body(
        ArticleBodyRequest(
            source="2607.13104v1",
            work_dir=tmp_path / "w",
            preference="hybrid",
            allow_network=False,
        )
    )
    assert result.route == "hybrid_deferred"
    assert result.body_path is None
    assert result.body_chars == 0


def test_s02_fitz_requires_injection(tmp_path: Path) -> None:
    pdf = tmp_path / "p.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    result = resolve_article_body(
        ArticleBodyRequest(
            source=str(pdf),
            work_dir=tmp_path / "w",
            preference="fitz",
            allow_network=False,
        )
    )
    assert result.route == "unavailable"
    assert "fitz_extract_not_injected" in result.diagnostics


def test_s03_single_article_pipeline_uses_body_route(tmp_path: Path) -> None:
    html = tmp_path / "paper.html"
    html.write_text(
        """<!doctype html><html><body>
        <h1>Composition Paper</h1>
        <p>Body text for chunks and readiness.</p>
        <h2>Method</h2>
        <p>Route must be html_native not hybrid.</p>
        </body></html>
        """,
        encoding="utf-8",
    )
    result = run_single_article_pipeline(
        SingleArticleRunRequest(
            source=str(html),
            work_dir=tmp_path / "run",
            mode="local",
            also_pdf=False,
            allow_network=False,
            repo_root=ROOT,
        )
    )
    assert result.body_route == "html_native"
    assert result.readiness is not None
    assert result.readiness.package.import_eligible is False
    assert result.to_dict()["hybrid_claimed_success"] is False
    assert result.to_dict()["body_route"] == "html_native"


def test_s05_resource_profile_on_decision_payload() -> None:
    d = decide_body_route(BodyRouteIntent(preference="auto", has_arxiv_id=True))
    assert d.route == "html_native"
    assert d.resource_profile.io_type == "network"
