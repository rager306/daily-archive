from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "run_m044_live_grobid_candidate_probe.py"
spec = importlib.util.spec_from_file_location("run_m044_live_grobid_candidate_probe", MODULE_PATH)
assert spec is not None
probe = importlib.util.module_from_spec(spec)
sys.modules["run_m044_live_grobid_candidate_probe"] = probe
assert spec.loader is not None
spec.loader.exec_module(probe)


def _target() -> dict:
    return {
        "articles": [
            {"article_key": "with-pdf", "m041_category": "baseline"},
            {"article_key": "missing-pdf", "m041_category": "reference_linked"},
        ]
    }


def _source(tmp_path: Path) -> dict:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    return {
        "records": [
            {
                "article_key": "with-pdf",
                "pdf_files": [str(pdf.relative_to(probe.ROOT))]
                if probe.ROOT in pdf.resolve().parents
                else [str(pdf)],
            },
            {"article_key": "missing-pdf", "pdf_files": []},
        ]
    }


def _source_under_root() -> dict:
    return {
        "records": [
            {"article_key": "with-pdf", "pdf_files": ["data/article_catalog/fake.pdf"]},
            {"article_key": "missing-pdf", "pdf_files": []},
        ]
    }


def _runtime(status: str = "live_ready") -> dict:
    return {"service_url": "http://localhost:8070", "current_grobid_status": status}


def _tei() -> bytes:
    return b"""
    <TEI xmlns="http://www.tei-c.org/ns/1.0">
      <teiHeader><fileDesc><titleStmt><title>Title</title></titleStmt></fileDesc></teiHeader>
      <text><body><div><p><ref target="x">x</ref></p></div><figure /></body><back><listBibl><biblStruct /></listBibl></back></text>
    </TEI>
    """


def test_summarize_tei_counts_and_hashes_without_raw_text():
    summary = probe.summarize_tei(_tei())

    assert summary["tei_byte_count"] > 0
    assert len(summary["tei_sha256"]) == 64
    assert summary["element_counts"]["biblStruct"] == 1
    assert summary["element_counts"]["div"] == 1
    assert summary["element_counts"]["ref"] == 1
    assert "tei_xml" not in summary


def test_build_live_grobid_packets_success_and_missing_pdf(monkeypatch, tmp_path):
    monkeypatch.setattr(probe, "ROOT", tmp_path)
    pdf = tmp_path / "data" / "article_catalog" / "fake.pdf"
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fake")
    source = _source_under_root()

    result = probe.build_live_grobid_packets(
        target=_target(),
        source_readiness=source,
        runtime_update=_runtime(),
        submitter=lambda _pdf, _url, _timeout: _tei(),
        run_guardrail_first=False,
    )

    assert result["status_counts"] == {"live_success": 1, "missing_pdf": 1}
    success = result["packets"][0]
    assert success["status"] == "live_success"
    assert success["element_counts"]["biblStruct"] == 1
    assert result["forbidden_payload_fields_absent"] is True
    assert result["graph_write_allowed"] is False


def test_build_live_grobid_packets_blocks_when_service_not_live(monkeypatch, tmp_path):
    monkeypatch.setattr(probe, "ROOT", tmp_path)
    pdf = tmp_path / "data" / "article_catalog" / "fake.pdf"
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fake")

    result = probe.build_live_grobid_packets(
        target=_target(),
        source_readiness=_source_under_root(),
        runtime_update=_runtime("blocked"),
        submitter=lambda _pdf, _url, _timeout: (_ for _ in ()).throw(
            AssertionError("must not submit")
        ),
        run_guardrail_first=False,
    )

    assert result["status_counts"] == {"service_blocked": 2}
    assert all(packet["blockers"] == ["grobid_service_not_live"] for packet in result["packets"])


def test_assert_no_forbidden_fields_rejects_raw_payload():
    try:
        probe.assert_no_forbidden_fields({"tei_xml": "<TEI>raw</TEI>"})
    except ValueError as exc:
        assert "forbidden fields" in str(exc)
    else:  # pragma: no cover - defensive failure branch
        raise AssertionError("expected ValueError")


def test_render_markdown_includes_no_raw_tei_statement(monkeypatch, tmp_path):
    monkeypatch.setattr(probe, "ROOT", tmp_path)
    pdf = tmp_path / "data" / "article_catalog" / "fake.pdf"
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fake")
    result = probe.build_live_grobid_packets(
        target=_target(),
        source_readiness=_source_under_root(),
        runtime_update=_runtime(),
        submitter=lambda _pdf, _url, _timeout: _tei(),
        run_guardrail_first=False,
    )

    markdown = probe.render_markdown(result)

    assert "Raw TEI/full text persisted: false" in markdown
    assert "live_success" in markdown
    assert "missing_pdf" in markdown
