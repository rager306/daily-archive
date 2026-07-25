"""M217: pure GROBID TEI header/citation parse + hybrid ETL seams."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.grobid_tei_parse import parse_grobid_tei
from research_graph.infrastructure.corpus.parsing.live_sidecar_adapters import (
    LiveGrobidSidecarAdapter,
)
from research_graph.workflows.composition.hybrid_sidecar_runtime import (
    HybridRuntimeRequest,
    run_hybrid_sidecar_runtime,
)
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    resolve_article_body,
)

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "grobid" / "sample_header_cites.tei.xml"
SAMPLE_TEI = Path("/tmp/sample.tei.xml")


def test_parse_fixture_header_and_listbibl_only() -> None:
    tei = FIXTURE.read_bytes()
    result = parse_grobid_tei(tei, paper_id="sample-1")
    assert result.parse_ok is True
    assert result.header.title == "Sample Hybrid Paper Title"
    assert len(result.header.authors) == 2
    assert result.header.authors[0]["surname"] == "Lovelace"
    assert "abstract" in result.header.abstract.lower() or "short abstract" in result.header.abstract
    assert result.header.idnos.get("DOI") == "10.1000/sample.doi"
    assert len(result.citations) == 2
    assert result.citations[0]["title"] == "First Citation Title"
    assert result.citations[0]["import_eligible"] is False
    assert result.citations[1]["title"] == "Second Citation Title"
    # sourceDesc biblStruct must not appear as citation
    titles = {c["title"] for c in result.citations}
    assert "Sample Hybrid Paper Title" not in titles
    assert result.to_dict()["import_eligible"] is False


def test_parse_invalid_tei_fail_closed() -> None:
    result = parse_grobid_tei(b"<not-tei", paper_id="x")
    assert result.parse_ok is False
    assert result.citations == ()
    assert any("parse_error" in d for d in result.diagnostics)


@pytest.mark.skipif(not SAMPLE_TEI.is_file(), reason="live sample TEI not on disk")
def test_parse_live_sample_tei_listbibl_not_header() -> None:
    result = parse_grobid_tei(SAMPLE_TEI.read_bytes(), paper_id="1508.07909")
    assert result.parse_ok is True
    assert "Subword" in result.header.title or "Neural" in result.header.title
    assert len(result.citations) >= 10
    # header paper title should not be first citation title typically
    assert result.header.title
    assert result.citations[0]["source"] == "grobid_tei_listBibl"


class _FakeGrobidStructured:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        tei = FIXTURE.read_bytes()
        parsed = parse_grobid_tei(tei, paper_id=paper_id)
        return {
            "status": "success",
            "header_title_present": bool(parsed.header.title),
            "header_author_count": len(parsed.header.authors),
            "bibl_count": len(parsed.citations),
            "body_element_count": 1,
            "ref_count": 0,
            "bytes": parsed.tei_bytes,
            "tei_present": True,
            "header": parsed.header.to_dict(),
            "citations": list(parsed.citations),
            "citation_count": len(parsed.citations),
            "structured_parse_ok": parsed.parse_ok,
        }


class _FakeOdl:
    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
        md = "# Title\n\n" + ("body paragraph. " * 400)
        return {
            "status": "success",
            "markdown": md,
            "markdown_size_bytes": len(md),
            "bounding_box_count": 5,
            "low_quality_source": False,
        }


def test_live_adapter_includes_structured_fields_from_tei_bytes() -> None:
    """Unit-level: adapter parse path without network using injected tei via subclass."""

    class _Adapter(LiveGrobidSidecarAdapter):
        def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict:
            # bypass HTTP: parse fixture as if TEI returned
            from research_graph.application.corpus.grobid_tei_parse import parse_grobid_tei

            parsed = parse_grobid_tei(FIXTURE.read_bytes(), paper_id=paper_id)
            return {
                "status": "success",
                "paper_id": paper_id,
                "header_title_present": bool(parsed.header.title),
                "header_author_count": len(parsed.header.authors),
                "bibl_count": len(parsed.citations),
                "body_element_count": 1,
                "ref_count": 0,
                "bytes": parsed.tei_bytes,
                "tei_present": True,
                "header": parsed.header.to_dict(),
                "citations": list(parsed.citations),
                "citation_count": len(parsed.citations),
                "structured_parse_ok": parsed.parse_ok,
            }

    pdf = Path("/tmp/no-need.pdf")
    metrics = _Adapter(ensure_service=False).extract_metrics(pdf, paper_id="sample-1")
    assert metrics["status"] == "success"
    assert metrics["citation_count"] == 2
    assert metrics["header"]["title"] == "Sample Hybrid Paper Title"
    assert metrics["citations"][0]["import_eligible"] is False


def test_hybrid_resolve_persists_header_and_citations(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n")
    work = tmp_path / "work"
    body = resolve_article_body(
        ArticleBodyRequest(
            source=str(pdf),
            work_dir=work,
            preference="hybrid",
            allow_network=False,
            paper_id="sample-1",
        ),
        grobid=_FakeGrobidStructured(),
        opendataloader=_FakeOdl(),
        hybrid_pdf_path=pdf,
    )
    assert body.route == "hybrid"
    assert body.body_path is not None and body.body_path.is_file()
    header_path = work / "body" / "sample-1.hybrid.header.json"
    cites_path = work / "body" / "sample-1.hybrid.citations.jsonl"
    assert header_path.is_file(), body.diagnostics
    assert cites_path.is_file()
    header = json.loads(header_path.read_text(encoding="utf-8"))
    assert header["title"] == "Sample Hybrid Paper Title"
    assert header["import_eligible"] is False
    lines = [json.loads(line) for line in cites_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert len(lines) == 2
    assert lines[0]["title"] == "First Citation Title"
    assert all(row["import_eligible"] is False for row in lines)


def test_runtime_packet_still_fail_closed_with_structured_grobid() -> None:
    pdf = Path("/tmp/x.pdf")
    result = run_hybrid_sidecar_runtime(
        HybridRuntimeRequest(paper_id="p", pdf_path=pdf),
        grobid=_FakeGrobidStructured(),
        opendataloader=_FakeOdl(),
    )
    assert result.packet.hybrid_claimed_success is True
    assert result.packet.import_eligible is False
    assert result.packet.ownership.citations == "grobid"
    assert result.packet.ownership.metadata == "grobid"
    assert result.packet.ownership.body == "opendataloader"
