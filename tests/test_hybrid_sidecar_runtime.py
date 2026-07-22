"""M212: hybrid sidecar merge + runtime composition (offline)."""

from __future__ import annotations

from pathlib import Path

import pytest

from research_graph.application.hybrid_sidecar import (
    BODY_MARKDOWN_MIN_CHARS,
    HybridCandidatePacket,
    decide_hybrid_runtime_route,
    merge_hybrid_sidecar_packets,
)
from research_graph.application.parser_body_route import BodyRouteIntent, decide_body_route
from research_graph.workflows.composition.hybrid_sidecar_runtime import (
    HybridRuntimeRequest,
    run_hybrid_sidecar_runtime,
)
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    resolve_article_body,
)
from research_graph.workflows.composition.single_article_pipeline import (
    SingleArticleRunRequest,
    run_single_article_pipeline,
)

ROOT = Path(__file__).resolve().parents[1]


def _fat_markdown(n: int = BODY_MARKDOWN_MIN_CHARS + 100) -> str:
    return "# Title\n\n" + ("body paragraph. " * (n // 16))


def test_s01_merge_hybrid_packet_from_metrics() -> None:
    md = _fat_markdown()
    packet = merge_hybrid_sidecar_packets(
        paper_id="2203.14465",
        grobid={"status": "success", "header_title_present": True, "bibl_count": 12},
        opendataloader={
            "status": "success",
            "markdown_size_bytes": len(md),
            "markdown": md,
            "bounding_box_count": 40,
            "low_quality_source": False,
        },
        body_markdown=md,
    )
    assert packet.route == "grobid_header_plus_opendataloader_body"
    assert packet.hybrid_claimed_success is True
    assert packet.ownership.metadata == "grobid"
    assert packet.ownership.body == "opendataloader"
    assert packet.import_eligible is False
    assert packet.graph_writes_allowed is False


def test_s01_merge_rejects_import_flags() -> None:
    with pytest.raises(ValueError, match="cannot authorize"):
        HybridCandidatePacket(
            paper_id="x",
            route="manual_review",
            ownership=merge_hybrid_sidecar_packets(
                paper_id="x", grobid=None, opendataloader=None
            ).ownership,
            body_markdown=None,
            body_chars=0,
            grobid_ok=False,
            odl_ok=False,
            odl_low_quality=False,
            confidence="low",
            import_eligible=True,
        )


def test_s02_route_grobid_fulltext_only_on_low_quality_odl() -> None:
    route, conf, diag = decide_hybrid_runtime_route(
        grobid_ok=True,
        odl_ok=True,
        odl_low_quality=True,
        body_chars=BODY_MARKDOWN_MIN_CHARS + 10,
    )
    assert route == "grobid_fulltext_only"
    assert "odl_low_quality" in diag


def test_s02_route_deferred_when_both_down() -> None:
    route, conf, diag = decide_hybrid_runtime_route(
        grobid_ok=False, odl_ok=False, odl_low_quality=False, body_chars=0
    )
    assert route == "deferred_unavailable"


def test_s03_runtime_without_ports_is_deferred() -> None:
    result = run_hybrid_sidecar_runtime(
        HybridRuntimeRequest(paper_id="p1", pdf_path=None)
    )
    assert result.packet.hybrid_claimed_success is False
    assert result.packet.route in {
        "deferred_unavailable",
        "manual_review",
        "grobid_fulltext_only",
        "opendataloader_only",
    }
    assert "grobid_port_not_injected" in result.diagnostics


class _FakeGrobid:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        return {
            "status": "success",
            "header_title_present": True,
            "bibl_count": 5,
            "arxiv_id": paper_id,
        }


class _FakeOdl:
    def __init__(self, markdown: str) -> None:
        self.markdown = markdown

    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        return {
            "status": "success",
            "markdown": self.markdown,
            "markdown_size_bytes": len(self.markdown),
            "bounding_box_count": 10,
            "low_quality_source": False,
        }


def test_s03_runtime_with_fakes_claims_hybrid(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    md = _fat_markdown()
    result = run_hybrid_sidecar_runtime(
        HybridRuntimeRequest(paper_id="2203.14465", pdf_path=pdf),
        grobid=_FakeGrobid(),
        opendataloader=_FakeOdl(md),
    )
    assert result.packet.hybrid_claimed_success is True
    assert result.packet.route == "grobid_header_plus_opendataloader_body"
    assert result.packet.body_chars >= BODY_MARKDOWN_MIN_CHARS


def test_s04_body_resolve_hybrid_success_with_ports(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    md = _fat_markdown()
    body = resolve_article_body(
        ArticleBodyRequest(
            source=str(pdf),
            work_dir=tmp_path / "w",
            preference="hybrid",
            allow_network=False,
        ),
        grobid=_FakeGrobid(),
        opendataloader=_FakeOdl(md),
        hybrid_pdf_path=pdf,
    )
    assert body.route == "hybrid"
    assert body.body_path is not None
    assert body.body_chars >= BODY_MARKDOWN_MIN_CHARS
    assert "hybrid_body_from_packet" in body.diagnostics


def test_s04_body_resolve_hybrid_deferred_without_ports(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    body = resolve_article_body(
        ArticleBodyRequest(
            source=str(pdf),
            work_dir=tmp_path / "w",
            preference="hybrid",
            allow_network=False,
        )
    )
    assert body.route == "hybrid_deferred"
    assert body.body_path is None
    assert body.decision.hybrid_claimed_success is False


def test_s04_policy_hybrid_available_attempts_hybrid_route() -> None:
    d = decide_body_route(
        BodyRouteIntent(preference="hybrid", hybrid_runtime_available=True)
    )
    assert d.route == "hybrid"
    assert d.hybrid_claimed_success is False  # policy never claims


def test_s05_single_article_hybrid_with_injection(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    md = _fat_markdown()
    result = run_single_article_pipeline(
        SingleArticleRunRequest(
            source=str(pdf),
            work_dir=tmp_path / "run",
            mode="hybrid",
            also_pdf=False,
            allow_network=False,
            repo_root=ROOT,
        ),
        grobid=_FakeGrobid(),
        opendataloader=_FakeOdl(md),
        hybrid_pdf_path=pdf,
    )
    assert result.body_route == "hybrid"
    assert result.to_dict()["hybrid_claimed_success"] is True
    assert result.readiness is not None
    assert result.readiness.package.import_eligible is False
    assert result.readiness.package.graph_writes_allowed is False
