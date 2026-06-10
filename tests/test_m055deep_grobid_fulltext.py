"""Tests for M055deep S01 GROBID fulltext re-benchmark."""

from __future__ import annotations

import hashlib
import json
import sys
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055deep_grobid_fulltext as fulltext  # noqa: E402
import compare_m055_header_vs_fulltext as compare_delta  # noqa: E402

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


class FakeResponse:
    def __init__(self, body: bytes = b"", status: int = 200) -> None:
        self._body = body
        self.status = status

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self) -> bytes:
        return self._body


def tei_payload(
    *,
    refs: int = 2,
    bibls: int = 2,
    sections: int = 2,
    formulas: int = 1,
    figures: int = 1,
    include_body: bool = True,
    padding: int = 1400,
) -> str:
    ref_xml = "".join("<ref>r</ref>" for _ in range(refs))
    bibl_xml = "".join(f"<biblStruct><analytic><title>ref {i}</title></analytic></biblStruct>" for i in range(bibls))
    formula_xml = "".join("<formula>x=y</formula>" for _ in range(formulas))
    figure_xml = "".join("<figure><head>f</head></figure>" for _ in range(figures))
    section_xml = "".join(
        f'<div type="section"><head>Section {i}</head><p>{"x" * padding}</p>{ref_xml}{formula_xml}{figure_xml}</div>'
        for i in range(sections)
    )
    body_xml = f"<text><body>{section_xml}</body></text>" if include_body else "<text />"
    return f"""<TEI xmlns="http://www.tei-c.org/ns/1.0">
      <teiHeader>
        <fileDesc>
          <titleStmt><title>Paper Title</title><author>Ada</author><author>Grace</author></titleStmt>
          <sourceDesc><listBibl>{bibl_xml}</listBibl></sourceDesc>
        </fileDesc>
        <profileDesc><abstract><p>Abstract text.</p></abstract></profileDesc>
      </teiHeader>
      {body_xml}
    </TEI>"""


def write_manifest(tmp_path: Path, entries: list[dict[str, object]]) -> Path:
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_text(json.dumps({"pdfs": entries}, indent=2), encoding="utf-8")
    return manifest_path


def write_pdf(tmp_path: Path, name: str = "paper.pdf") -> tuple[Path, str]:
    pdf_path = tmp_path / name
    pdf_bytes = b"%PDF-1.4\nfake pdf bytes\n%%EOF\n"
    pdf_path.write_bytes(pdf_bytes)
    return pdf_path, hashlib.sha256(pdf_bytes).hexdigest()


def packet(arxiv_id: str, **values: object) -> dict[str, object]:
    base: dict[str, object] = {
        "arxiv_id": arxiv_id,
        "status": "success",
        "body_element_count": 0,
        "ref_count": 0,
        "bibl_count": 0,
        "equation_count": 0,
        "figure_count": 0,
    }
    base.update(values)
    return base


def test_probe_grobid_fulltext_success_on_200_ok(tmp_path: Path) -> None:
    pdf_path, _ = write_pdf(tmp_path)
    tei = tei_payload()
    with mock.patch.object(fulltext.urllib.request, "urlopen", return_value=FakeResponse(tei.encode("utf-8"), 200)):
        result = fulltext._probe_grobid_fulltext(pdf_path, "http://127.0.0.1:8070/api/processFulltextDocument")

    assert result["http_status"] == 200
    assert result["tei_text"] == tei
    assert result["bytes"] == len(tei.encode("utf-8"))
    assert result["error"] is None


def test_probe_grobid_fulltext_fail_closed_when_grobid_down(tmp_path: Path) -> None:
    pdf_path, _ = write_pdf(tmp_path)
    with mock.patch.object(fulltext.urllib.request, "urlopen", side_effect=urllib.error.URLError("down")):
        result = fulltext._probe_grobid_fulltext(pdf_path, "http://127.0.0.1:8070/api/processFulltextDocument")

    assert result["tei_text"] == ""
    assert result["http_status"] is None
    assert result["bytes"] == 0
    assert "URLError" in result["error"]


def test_extract_fulltext_metrics_body_elements_are_positive() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(sections=2))

    assert metrics["body_element_count"] > 0
    assert metrics["section_count"] == 2
    assert metrics["header_title_present"] is True
    assert metrics["header_author_count"] == 2
    assert metrics["abstract_present"] is True


def test_extract_fulltext_metrics_counts_untyped_grobid_body_divs_as_sections() -> None:
    tei = tei_payload(sections=1).replace('type="section"', '')

    metrics = fulltext._extract_fulltext_metrics(tei)

    assert metrics["body_element_count"] > 0
    assert metrics["section_count"] == 1


def test_extract_fulltext_metrics_counts_formula_tags_as_equations() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(sections=1, formulas=3, figures=2))

    assert metrics["equation_count"] == 3
    assert metrics["figure_count"] == 2


def test_low_quality_source_criteria_rejects_small_tei() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(padding=1))
    metrics["tei_size_bytes"] = 100

    assert fulltext._low_quality_source_criteria(metrics) is True


def test_low_quality_source_criteria_rejects_zero_refs() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(refs=0))

    assert fulltext._low_quality_source_criteria(metrics) is True


def test_low_quality_source_criteria_rejects_zero_body_elements() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(include_body=False))

    assert fulltext._low_quality_source_criteria(metrics) is True


def test_low_quality_source_criteria_rejects_zero_sections() -> None:
    metrics = fulltext._extract_fulltext_metrics(tei_payload(sections=0))
    metrics["tei_size_bytes"] = 2048
    metrics["ref_count"] = 1
    metrics["body_element_count"] = 1

    assert fulltext._low_quality_source_criteria(metrics) is True


def test_probe_grobid_fulltext_writes_aggregate_counts(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    pdf_path, sha = write_pdf(tmp_path)
    manifest_path = write_manifest(
        tmp_path,
        [{"arxiv_id": "1234.5678", "article_key": "1234.5678", "category": "cs-cl", "path": str(pdf_path), "sha256": sha}],
    )
    monkeypatch.setattr(fulltext, "_utc_now", lambda: "2026-06-10T00:00:00+00:00")
    monkeypatch.setattr(
        fulltext,
        "_probe_grobid_fulltext",
        lambda *args, **kwargs: {
            "tei_text": tei_payload(refs=2, bibls=3, sections=2, formulas=1, figures=1),
            "http_status": 200,
            "bytes": 4096,
            "duration_ms": 12,
            "error": None,
        },
    )

    summary = fulltext.probe_grobid_fulltext(manifest_path, tmp_path / "out", grobid_url="http://127.0.0.1:8070")

    assert summary["total_pdfs"] == 1
    assert summary["success_count"] == 1
    assert summary["body_positive_count"] == 1
    assert summary["total_ref_count"] == 4
    assert (tmp_path / "out" / "per-pdf" / "1234.5678.json").exists()
    assert (tmp_path / "out" / "tei" / "1234.5678.tei.xml").exists()


def test_safety_defaults_are_all_false_in_summary_and_packet(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    pdf_path, sha = write_pdf(tmp_path)
    manifest_path = write_manifest(tmp_path, [{"arxiv_id": "1", "path": str(pdf_path), "sha256": sha}])
    monkeypatch.setattr(fulltext, "_probe_grobid_fulltext", lambda *args, **kwargs: {"tei_text": tei_payload(), "http_status": 200, "bytes": 1, "duration_ms": 1, "error": None})

    summary = fulltext.probe_grobid_fulltext(manifest_path, tmp_path / "out", grobid_url="http://127.0.0.1:8070")
    packet_payload = json.loads((tmp_path / "out" / "per-pdf" / "1.json").read_text(encoding="utf-8"))

    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert set(packet_payload["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in packet_payload["safety_defaults"].values())


def test_m022_repair_candidate_is_set_on_low_quality_source(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    pdf_path, sha = write_pdf(tmp_path)
    manifest_path = write_manifest(tmp_path, [{"arxiv_id": "low", "path": str(pdf_path), "sha256": sha}])
    monkeypatch.setattr(fulltext, "_probe_grobid_fulltext", lambda *args, **kwargs: {"tei_text": tei_payload(refs=0), "http_status": 200, "bytes": 1, "duration_ms": 1, "error": None})

    fulltext.probe_grobid_fulltext(manifest_path, tmp_path / "out", grobid_url="http://127.0.0.1:8070")
    packet_payload = json.loads((tmp_path / "out" / "per-pdf" / "low.json").read_text(encoding="utf-8"))

    assert packet_payload["status"] == "low_quality_source"
    assert packet_payload["low_quality_source"] is True
    assert packet_payload["m022_repair_candidate"] is True


def test_compare_header_vs_fulltext_emits_per_pdf_deltas(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    header_dir = tmp_path / "header"
    fulltext_dir = tmp_path / "fulltext"
    header_dir.mkdir()
    fulltext_dir.mkdir()
    (header_dir / "a.json").write_text(json.dumps(packet("a", body_element_count=0, ref_count=1, bibl_count=1)), encoding="utf-8")
    (fulltext_dir / "a.json").write_text(
        json.dumps(packet("a", body_element_count=10, ref_count=5, bibl_count=4, equation_count=2, figure_count=3)),
        encoding="utf-8",
    )
    monkeypatch.setattr(compare_delta, "_utc_now", lambda: "2026-06-10T00:00:00+00:00")

    payload = compare_delta.compare_header_vs_fulltext(header_dir, fulltext_dir, tmp_path / "delta.json")

    assert payload["per_pdf"] == [
        {
            "arxiv_id": "a",
            "header_status": "success",
            "fulltext_status": "success",
            "header_body_element_count": 0,
            "fulltext_body_element_count": 10,
            "body_delta": 10,
            "header_ref_count": 1,
            "fulltext_ref_count": 5,
            "ref_delta": 4,
            "header_bibl_count": 1,
            "fulltext_bibl_count": 4,
            "bibl_delta": 3,
            "header_equation_count": 0,
            "fulltext_equation_count": 2,
            "equation_delta": 2,
            "header_figure_count": 0,
            "fulltext_figure_count": 3,
            "figure_delta": 3,
        }
    ]
    assert payload["aggregate"]["body_delta_total"] == 10
    assert (tmp_path / "delta.json").exists()


def test_idempotent_summary_when_inputs_are_unchanged(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    pdf_path, sha = write_pdf(tmp_path)
    manifest_path = write_manifest(tmp_path, [{"arxiv_id": "stable", "path": str(pdf_path), "sha256": sha}])
    monkeypatch.setattr(fulltext, "_utc_now", lambda: "2026-06-10T00:00:00+00:00")
    monkeypatch.setattr(fulltext, "_probe_grobid_fulltext", lambda *args, **kwargs: {"tei_text": tei_payload(), "http_status": 200, "bytes": 1, "duration_ms": 1, "error": None})

    first = fulltext.probe_grobid_fulltext(manifest_path, tmp_path / "out", grobid_url="http://127.0.0.1:8070")
    second = fulltext.probe_grobid_fulltext(manifest_path, tmp_path / "out", grobid_url="http://127.0.0.1:8070")

    assert second == first
    assert json.loads((tmp_path / "out" / "summary.json").read_text(encoding="utf-8")) == first


def test_existing_regression_test_modules_for_m050_through_m055_are_present() -> None:
    expected = [
        "tests/test_m050_article_artifact_reducer.py",
        "tests/test_m050_article_artifact_worker.py",
        "tests/test_m050_e2e_pipeline.py",
        "tests/test_m052_rlm_workflow.py",
        "tests/test_m053_audit_s02.py",
        "tests/test_m053_grobid_pilot.py",
        "tests/test_m055_benchmark_s01.py",
        "tests/test_m055_benchmark_s02.py",
        "tests/test_m055_benchmark_s03.py",
        "tests/test_m055_benchmark_s04.py",
        "tests/test_m055_benchmark_s05.py",
    ]

    assert all((ROOT / path).exists() for path in expected)
